(()=>{
 const host=document.getElementById('review-page');if(!host||document.getElementById('shoot-job-panel'))return;
 const section=document.createElement('section');section.className='panel';section.id='shoot-job-panel';
 section.innerHTML=`<h2>Cull Through Review</h2><p class="meta">Run a local JPEG or CR3 shoot. Originals and historical ratings stay unchanged.</p>
 <form id="shoot-form" class="shoot-form"><label>Source shoot folder<div class="shoot-path"><input id="shoot-source" required placeholder="Choose a shoot folder" autocomplete="off"><button type="button" id="shoot-source-browse">Browse</button></div></label>
 <label>Output folder root<div class="shoot-path"><input id="shoot-output" required autocomplete="off"><button type="button" id="shoot-output-browse">Browse</button></div></label>
 <label>Shoot content<select id="shoot-kind"><option value="aviation">Aviation</option><option value="non-aviation">Non-aviation</option></select></label>
 <p class="meta">Collect native measurements, registration suggestions and existing identity evidence. Photos without a completed visual assessment stay Needs review. Final rubric grades determine output folders; HyperIQA is diagnostic only.</p>
 <button type="submit" id="shoot-start" disabled>Start Cull Through Review</button></form>
 <p id="shoot-message" class="meta" role="status" aria-live="polite">Connecting to local Rust worker…</p>
 <div id="shoot-jobs"></div><div id="shoot-details" hidden></div>`;
 host.prepend(section);
 const get=id=>document.getElementById(id);let token=null,busy=false,selected=null,lastRendered='',submissionId=null,refreshing=false;
 const text=(tag,value,cls)=>{const el=document.createElement(tag);el.textContent=value;if(cls)el.className=cls;return el;};
 if(location.protocol==='file:'){const link=text('a','Open the live local dashboard to process a shoot');link.href='http://127.0.0.1:47831/#review';get('shoot-message').replaceChildren(link);return;}
 async function api(route,body){const response=await fetch('/shoots/'+route,{method:body===undefined?'GET':'POST',headers:body===undefined?{}:{'Content-Type':'application/json','X-Shoot-Token':token},body:body===undefined?undefined:JSON.stringify(body),signal:AbortSignal.timeout(route==='pick-folder'?185000:35000)});const result=await response.json();if(!response.ok)throw Error(result.error||'Shoot worker unavailable');return result;}
 const message=value=>get('shoot-message').textContent=value;
 function action(label,handler,key){const button=text('button',label);button.type='button';if(key)button.dataset.shootFocus=key;button.addEventListener('click',handler);return button;}
 function download(name,value,type='application/json'){const url=URL.createObjectURL(new Blob([typeof value==='string'?value:JSON.stringify(value,null,2)],{type}));const link=document.createElement('a');link.href=url;link.download=name;link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
 function csv(rows){const keys=[...new Set(rows.flatMap(r=>Object.keys(r)))];const cell=v=>{let s=v==null?'':typeof v==='object'?JSON.stringify(v):String(v);if(/^[=+@\-\t\r]/.test(s))s="'"+s;return '"'+s.replaceAll('"','""')+'"';};return [keys.map(cell).join(','),...rows.map(r=>keys.map(k=>cell(r[k])).join(','))].join('\r\n');}
 function evidenceView(item){const fragment=document.createDocumentFragment(),rubric=item.assessment?.rubric,evidence=item.evidence;
  if(rubric?.status==='rated'){
   fragment.append(text('p',`Rubric 14: ${rubric.rawScore} subtotal → ${rubric.technicalScore} technical → ${rubric.finalScore} final · ${rubric.motionMode} · ${rubric.completeness} · ${rubric.confidence} confidence (${rubric.uncertaintyRange.join('–')})`));
   fragment.append(text('p','Primary flaw: '+rubric.primaryFlaw),text('p','Strongest attribute: '+rubric.strongestAttribute));
   for(const [category,score] of Object.entries(rubric.categories))fragment.append(text('p',`${category}: ${score} — ${rubric.categoryEvidence[category]}`,'meta'));
   for(const gate of rubric.gates)fragment.append(text('p',`Ceiling ${gate.ceiling}: ${gate.reason}`,'meta'));
   if(rubric.wholeAircraftCapApplied)fragment.append(text('p','Final score capped at 79: aircraft cut off by image boundary.','meta'));
   fragment.append(text('p','Zones: '+Object.entries(rubric.zones).map(([k,v])=>`${k}: ${v}`).join(' · '),'meta'));
   for(const defect of rubric.defectLedger)fragment.append(text('p',`${defect.severity} ${defect.kind} → ${defect.owningCategory}: ${defect.evidence}`,'meta'));
  }else fragment.append(text('p',rubric?.reason||'Evidence has not been collected yet.','meta'));
  if(item.labels)fragment.append(text('p','Identification: '+item.labels.join(' / '),'meta'));
  const lines=evidence?.registrationOcr?.lines||[];
  if(lines.length)fragment.append(text('p','OCR suggestions (unconfirmed): '+lines.map(l=>l.rawText).join(' · '),'meta'));
  const tiles=evidence?.technical?.tiles||[];
  if(tiles.length)fragment.append(text('p',`${tiles.length} native full-frame measurement tiles collected. Background is included; these are not aircraft-zone grades.`,'meta'));
  for(const error of evidence?.errors||[])fragment.append(text('p',error,'meta'));
  const diagnostic=item.assessment?.modelDiagnostic?.rawQualityScore;
  if(diagnostic!=null)fragment.append(text('p',`HyperIQA diagnostic: ${diagnostic.toFixed(1)} (not used for naming or routing)`,'meta'));
  return fragment;
 }
 async function showDetails(id){try{selected=id;const detail=await api('jobs/'+id);const box=get('shoot-details');
  // Do not replace focused controls or expanded evidence on unchanged polling responses.
  const encoded=JSON.stringify(detail);if(box.dataset.snapshot===encoded)return;box.dataset.snapshot=encoded;
  const expanded=new Set([...box.querySelectorAll('details[open]')].map(d=>d.dataset.ordinal));
  box.hidden=false;box.replaceChildren(text('h3','Shoot results'),text('p','Rubric grades use reviewed evidence. Score comparisons and the full-shoot test remain deferred.','meta'));
  const actions=document.createElement('div');actions.className='shoot-actions';
  actions.append(action('Download shoot report',()=>download(id+'-results.json',detail)),action('Download manifest CSV',()=>download(id+'-manifest.csv',csv(detail.manifest||[]),'text/csv;charset=utf-8')));
  if(!['queued','running','cancel_requested'].includes(detail.job.state)){
   const input=document.createElement('input');input.type='file';input.accept='.json,application/json';input.hidden=true;
   input.addEventListener('change',async()=>{const file=input.files[0];if(!file)return;try{if(file.size>65536)throw Error('Review file exceeds 64 KB');await api('jobs/'+id+'/review',JSON.parse(await file.text()));message('Visual review accepted. Resume the shoot to apply its final grade and create verified output copies.');box.dataset.snapshot='';await refresh();}catch(error){message(error.message);}finally{input.value='';}});
   actions.append(action('Load completed visual review',()=>input.click()),input);
  }box.append(actions);
  const wrap=document.createElement('div');wrap.className='table-wrap';const table=document.createElement('table');const head=document.createElement('thead');const hr=document.createElement('tr');for(const title of ['Photo','State','Final rubric grade','Output / issue'])hr.append(text('th',title));head.append(hr);table.append(head);const body=document.createElement('tbody');
  for(const item of detail.items){const row=document.createElement('tr');const photo=document.createElement('td');const disclosure=document.createElement('details');disclosure.dataset.ordinal=String(item.ordinal);disclosure.open=expanded.has(String(item.ordinal));disclosure.append(text('summary',item.source.split(/[\\/]/).pop()),evidenceView(item));photo.append(disclosure);row.append(photo,text('td',item.state.replaceAll('_',' ')),text('td',item.score==null?'Unrated':String(item.score)),text('td',item.error||item.output||(item.state==='needs_review'?'Needs visual assessment; no output copy created':'Pending')));body.append(row);}table.append(body);wrap.append(table);box.append(wrap);
 }catch(error){message(error.message);}}
 function render(result){const encoded=JSON.stringify(result);if(encoded===lastRendered)return;lastRendered=encoded;const focused=document.activeElement?.dataset.shootFocus;const box=get('shoot-jobs');box.replaceChildren();busy=result.jobs.some(({job})=>['queued','running','cancel_requested'].includes(job.state));get('shoot-start').disabled=!token||busy;
  for(const record of result.jobs){const {job}=record;const article=document.createElement('article');article.className='shoot-job';article.append(text('h3',job.source.split(/[\\/]/).filter(Boolean).pop()||'Shoot'),text('p',`${record.processed} / ${job.total} processed · ${record.completed} verified outputs · ${record.failed} errors · ${record.needsReview||0} need visual review · ${record.needsIdentification} need identification`,'meta'));
   const progress=document.createElement('progress');progress.max=job.total;progress.value=record.processed;progress.setAttribute('aria-label','Shoot progress');article.append(progress,text('p',job.state.replaceAll('_',' '),'meta'));
   const destinations=Object.entries(record.destinations).map(([folder,count])=>`${folder}: ${count}`).join(' · ');if(destinations)article.append(text('p',destinations,'meta'));if(job.error)article.append(text('p',job.error,'meta'));if(job.notification)article.append(text('p','Notification: '+job.notification,'meta'));
   const buttons=document.createElement('div');buttons.className='shoot-actions';buttons.append(action('View results',()=>showDetails(job.id),job.id+'-results'));
   if(['running','queued'].includes(job.state))buttons.append(action('Cancel after current photo',async()=>{try{await api('jobs/'+job.id+'/cancel',{});await refresh();}catch(error){message(error.message);}},job.id+'-cancel'));
   if(['paused','cancelled','completed_with_errors','awaiting_review'].includes(job.state)&&!busy)buttons.append(action('Resume / retry unfinished',async()=>{try{await api('jobs/'+job.id+'/resume',{});await refresh();}catch(error){message(error.message);}},job.id+'-resume'));
   article.append(buttons);box.append(article);
  }if(focused){const button=[...box.querySelectorAll('button')].find(b=>b.dataset.shootFocus===focused)||[...box.querySelectorAll('button')].find(b=>b.dataset.shootFocus===focused.replace(/-(?:cancel|resume)$/,'-results'));button?.focus();}
 }
 async function refresh(){if(refreshing)return;refreshing=true;try{render(await api('jobs'));if(selected)await showDetails(selected);}catch(error){message(error.message);}finally{refreshing=false;}}
 for(const field of ['source','output'])get('shoot-'+field+'-browse').addEventListener('click',async()=>{const button=get('shoot-'+field+'-browse');button.disabled=true;try{const chosen=await api('pick-folder',{});if(chosen.path){get('shoot-'+field).value=chosen.path;submissionId=null;}}catch(error){message(error.message);}finally{button.disabled=false;}});
 for(const field of ['shoot-source','shoot-output','shoot-kind'])get(field).addEventListener('change',()=>submissionId=null);
 get('shoot-form').addEventListener('submit',async event=>{event.preventDefault();if(!token||busy)return;get('shoot-start').disabled=true;try{const created=await api('jobs',{source:get('shoot-source').value.trim(),output:get('shoot-output').value.trim(),kind:get('shoot-kind').value,requestId:submissionId||(submissionId=crypto.randomUUID())});submissionId=null;selected=created.job.id;message('Shoot queued. You can leave this dashboard open while Rust processes it.');await refresh();}catch(error){message(error.message);get('shoot-start').disabled=busy;}});
 async function connect(){try{const config=await api('config');token=config.clientToken;if(!get('shoot-output').value)get('shoot-output').value=config.defaultOutput;get('shoot-start').disabled=busy;message('Local Rust worker ready. Select a shoot to start.');await refresh();}catch(error){message(error.message);}}
 connect();setInterval(()=>{if(document.hidden)return;if(token)refresh();else connect();},3000);
})();
