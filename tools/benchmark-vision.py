"""Isolated CPU encoder benchmark: generated image only, no photo grades or shoot jobs."""
import argparse,base64,hashlib,io,json,secrets,subprocess,time,urllib.request,urllib.error
from pathlib import Path
from PIL import Image,ImageDraw
p=argparse.ArgumentParser();p.add_argument('manifest',type=Path);p.add_argument('--cases',default='f16-4,q8-4,q8-6');p.add_argument('--min-image-tokens',type=int);p.add_argument('--max-image-tokens',type=int);p.add_argument('--size',type=int,default=256);args=p.parse_args()
manifest=args.manifest.resolve();record=json.loads(manifest.read_text(encoding='utf-8-sig'));root=manifest.parent/'benchmarks';root.mkdir(exist_ok=True)
keyfile=root/'api-key.txt'
if not keyfile.exists():keyfile.write_text(secrets.token_hex(32),encoding='ascii')
key=keyfile.read_text().strip();http=urllib.request.build_opener(urllib.request.ProxyHandler({}));url='http://127.0.0.1:47836'
image=Image.new('RGB',(args.size,args.size),'white');ImageDraw.Draw(image).rectangle((args.size//8,args.size//8,args.size*7//8,args.size*7//8),fill='red');stream=io.BytesIO();image.save(stream,format='PNG');data='data:image/png;base64,'+base64.b64encode(stream.getvalue()).decode()
schema={'type':'object','properties':{'color':{'type':'string','enum':['red','blue','green']},'shape':{'type':'string','enum':['rectangle','circle']}},'required':['color','shape'],'additionalProperties':False}
def request(path,payload=None,timeout=5):
 req=urllib.request.Request(url+path,data=None if payload is None else json.dumps(payload).encode(),headers={'Content-Type':'application/json','Authorization':'Bearer '+key})
 with http.open(req,timeout=timeout)as response:return json.load(response)
try:
 request('/health');raise RuntimeError('Benchmark port is occupied; refusing to interfere')
except urllib.error.URLError:pass
results=[]
for case in args.cases.split(','):
 precision,threads=case.split('-');threads=int(threads)
 projector=Path(record['projector']).parent/('mmproj-Qwen3VL-2B-Instruct-'+('F16' if precision=='f16' else 'Q8_0')+'.gguf')
 expected={'f16':'c3d5afbef5287953acd57b4043d2269456e5761a4eaccb3b71b062996970aea5','q8':'f9a68fabba69c3b81e153367b2c7521030b0fa8bb0de400c9599c8e6725f9c82'}[precision]
 with projector.open('rb')as f:assert hashlib.file_digest(f,'sha256').hexdigest()==expected
 argv=[record['server'],'-m',record['model'],'--mmproj',str(projector),'--host','127.0.0.1','--port','47836','--api-key-file',str(keyfile),'--alias','synthetic-vision-benchmark','--ctx-size','4096','--parallel','1','--threads',str(threads),'--threads-batch',str(threads),'--n-gpu-layers','0','--no-mmproj-offload','--no-webui','--jinja']
 if args.min_image_tokens is not None:argv+=['--image-min-tokens',str(args.min_image_tokens)]
 if args.max_image_tokens is not None:argv+=['--image-max-tokens',str(args.max_image_tokens)]
 label=record['runtimeVersion']+'-'+case+'-s'+str(args.size)
 load_started=time.perf_counter()
 with(root/(label+'.log')).open('wb')as log:child=subprocess.Popen(argv,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0),cwd=str(Path(record['server']).parent))
 try:
  deadline=time.monotonic()+180
  while True:
   if child.poll()is not None:raise RuntimeError('Benchmark runtime failed; inspect '+case+'.log')
   try:request('/health');break
   except OSError:
    if time.monotonic()>deadline:raise RuntimeError('Model startup timed out')
    time.sleep(.2)
  startup=time.perf_counter()-load_started
  started=time.perf_counter();response=request('/v1/chat/completions',{'model':'synthetic-vision-benchmark','messages':[{'role':'user','content':[{'type':'text','text':'Identify the color and shape of the large colored object on the white background.'},{'type':'image_url','image_url':{'url':data}}]}],'temperature':0,'seed':42,'max_tokens':80,'cache_prompt':False,'response_format':{'type':'json_object','schema':schema}},timeout=900)
  seconds=time.perf_counter()-started;choice=response['choices'][0];result=json.loads(choice['message']['content']);assert choice['finish_reason']=='stop' and result=={'color':'red','shape':'rectangle'},result
  row={'case':case,'runtime':record['runtimeVersion'],'imageSize':args.size,'minImageTokens':args.min_image_tokens,'maxImageTokens':args.max_image_tokens,'startupSeconds':round(startup,3),'inferenceSeconds':round(seconds,3),'usage':response.get('usage'),'result':result,'generatedImageOnly':True,'projectorSha256':expected};results.append(row)
  (root/('results-'+record['runtimeVersion']+'-s'+str(args.size)+'.json')).write_text(json.dumps(results,indent=2));print(json.dumps(row),flush=True)
 finally:
  child.terminate()
  try:child.wait(timeout=10)
  except subprocess.TimeoutExpired:child.kill();child.wait()
