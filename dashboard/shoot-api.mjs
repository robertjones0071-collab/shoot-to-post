// Adapter for the existing local dashboard. Processing and job persistence live in Rust.
import {spawn} from 'node:child_process';
import {randomBytes} from 'node:crypto';
import {readFile,writeFile,mkdir,readdir,access} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import path from 'node:path';

const project=fileURLToPath(new URL('../',import.meta.url));
const settingsFile=path.join(project,'.local/worker.json');
const workerUrl='http://127.0.0.1:47834';
const clientToken=randomBytes(24).toString('hex');
let setupPromise,settings,child,lastWorkerError=null;
async function configure(){
 if(setupPromise)return setupPromise;
 setupPromise=(async()=>{
  try{settings=JSON.parse((await readFile(settingsFile,'utf8')).replace(/^\uFEFF/,''));}
  catch(error){
   if(error.code!=='ENOENT')throw error;
   const dashboard=JSON.parse((await readFile(path.join(project,'../aviation-dashboard/config.local.json'),'utf8')).replace(/^\uFEFF/,''));
   const root=dashboard.photoRoot;
   const entries=await readdir(root,{withFileTypes:true});const folderNames={};
   for(const number of [2,3,4,5,6]){const matching=entries.filter(e=>e.isDirectory()&&e.name.startsWith(number+'. '));if(matching.length!==1)throw Error('Output folder role '+number+' is unavailable or ambiguous');folderNames[number]=matching[0].name;}
   const model=JSON.parse(await readFile(path.join(project,'.local/hyperiqa.source.json'),'utf8'));
   settings={bind:'127.0.0.1:47834',token:randomBytes(32).toString('hex'),database:path.join(project,'.local/jobs.sqlite'),allowedRoots:[root],
    model:path.join(project,'.local/hyperiqa.onnx'),modelSha256:model.onnxSha256,
    runtime:path.join(project,'.local/export-packages/onnxruntime/capi/onnxruntime.dll'),rawDecoder:path.join(project,'.local/libraw/LibRaw-0.22.3/bin/dcraw_emu.exe'),
    defaultOutput:root,folderNames};
   await mkdir(path.dirname(settingsFile),{recursive:true});await writeFile(settingsFile,JSON.stringify(settings,null,2),{flag:'wx'});
  }
  return settings;
 })().catch(error=>{setupPromise=null;throw error;});return setupPromise;
}
async function callWorker(route,options={},timeout=5000){
 const config=await configure();
 const response=await fetch(workerUrl+route,{...options,headers:{Authorization:'Bearer '+config.token,...options.headers},signal:AbortSignal.timeout(timeout)});
 const value=await response.json();if(!response.ok)throw Object.assign(Error(value.error||'Shoot worker request failed'),{status:response.status});return value;
}
async function ready(){
 await configure();try{return await callWorker('/health');}catch{}
 if(!child||child.exitCode!==null||!child.pid){
  const candidates=['target/x86_64-pc-windows-gnu/release/shoot-to-post-worker.exe','target/release/shoot-to-post-worker.exe'].map(p=>path.join(project,p));let executable;
  for(const value of candidates){try{await access(value);executable=value;break;}catch{}}
  if(!executable)throw Error('Build the Rust shoot worker before starting a shoot');
  child=spawn(executable,[settingsFile],{windowsHide:true,cwd:project,env:{...process.env,PATH:path.join(project,'.local/sqlite')+path.delimiter+(process.env.PATH||'')},stdio:['ignore','ignore','pipe']});
  child.stderr.on('data',chunk=>lastWorkerError=((lastWorkerError||'')+chunk.toString()).slice(-3000));child.on('error',error=>lastWorkerError=error.message);
 }
 for(let i=0;i<40;i++){try{return await callWorker('/health',{},1000);}catch{}await new Promise(r=>setTimeout(r,200));}
 throw Error(lastWorkerError||'Shoot worker did not become ready');
}
function send(res,status,value){res.writeHead(status,{'Content-Type':'application/json','Cache-Control':'no-store'});res.end(JSON.stringify(value));}
export async function handleShootRequest(req,res,url){
 if(!url.pathname.startsWith('/shoots/'))return false;
 try{
  if(!['127.0.0.1:47831','localhost:47831'].includes(req.headers.host)||req.headers['sec-fetch-site']==='cross-site'||req.headers.origin==='null')throw Object.assign(Error('Use the live local dashboard for shoot processing'),{status:403});
  if(!['GET','POST'].includes(req.method))throw Object.assign(Error('Method not allowed'),{status:405});
  if(req.method==='POST'&&req.headers['x-shoot-token']!==clientToken)throw Object.assign(Error('Reload the dashboard before starting or changing a shoot'),{status:403});
  await ready();
  if(url.pathname==='/shoots/config'&&req.method==='GET'){send(res,200,{...await callWorker('/settings'),clientToken});return true;}
  if(url.pathname==='/shoots/health'&&req.method==='GET'){send(res,200,await callWorker('/health'));return true;}
  const route=url.pathname.slice('/shoots'.length);
  if(!/^\/(?:jobs(?:\/shoot-[a-zA-Z0-9-]+(?:\/(?:cancel|resume))?)?|pick-folder)$/.test(route))throw Object.assign(Error('Unknown shoot operation'),{status:404});
  let body;if(req.method==='POST'){body='';for await(const chunk of req){body+=chunk;if(Buffer.byteLength(body)>16384)throw Object.assign(Error('Request too large'),{status:400});}}
  send(res,200,await callWorker(route,{method:req.method,body,headers:body?{'Content-Type':'application/json'}:{}},route==='/pick-folder'?180000:30000));
 }catch(error){send(res,error.status||503,{error:error.message});}return true;
}
