(()=>{
 const host=document.getElementById('review-page');if(!host||document.getElementById('shoot-job-panel'))return;
 const section=document.createElement('section');section.className='panel';section.id='shoot-job-panel';
 section.innerHTML=`<h2>Cull Through Review</h2><p class="meta">Run a local JPEG or CR3 shoot. Originals and historical ratings stay unchanged.</p>
 <form id="shoot-form" class="shoot-form"><label>Source shoot folder<div class="shoot-path"><input id="shoot-source" required placeholder="Choose a shoot folder" autocomplete="off"><button type="button" id="shoot-source-browse">Browse</button></div></label>
 <label>Output folder root<div class="shoot-path"><input id="shoot-output" required autocomplete="off"><button type="button" id="shoot-output-browse">Browse</button></div></label>
 <label>Shoot content<select id="shoot-kind"><option value="aviation">Aviation</option><option value="non-aviation">Non-aviation</option></select></label>
 <p class="meta">Provisional HyperIQA scores determine output folders for this run. Existing filename labels are reused; missing identification stays Unknown. Score calibration is deferred.</p>
 <button type="submit" id="shoot-start" disabled>Start Cull Through Review</button></form>
 <p id="shoot-message" class="meta" role="status" aria-live="polite">Connecting to local Rust worker…</p>
 <div id="shoot-jobs"></div><div id="shoot-details" hidden></div>`;
 host.prepend(section);
 const get=id=>document.getElementById(id);let token=null,busy=false,selected=null,lastRendered='',submissionId=null;
 const text=(tag,value,cls)=>{const el=document.createElement(tag);el.textContent=value;if(cls)el.className=cls;return el;};
 if(location.protocol==='file:'){const link=text('a','Open the live local dashboard to process a shoot');link.href='http://127.0.0.1:47831/#review';get('shoot-message').replaceChildren(link);return;}
 async function api(route,body){const response=await fetch('/shoots/'+route,{method:body===undefined?'GET':'POST',headers:body===undefined?{}:{'Content-Type':'application/json','X-Shoot-Token':token},body:body===undefined?undefined:JSON.stringify(body),signal:AbortSignal.timeout(route==='pick-folder'?185000:35000)});const result=await response.json();if(!response.ok)throw Error(result.error||'Shoot worker unavailable');return result;}
 const message=value=>get('shoot-message').textContent=value;
 function action(label,handler,key){const button=text('button',label);button.type='button';if(key)button.dataset.shootFocus=key;button.addEventListener('click',handler);return button;}
 async function showDetails(id){try{selected=id;const detail=await api('jobs/'+id);const box=get('shoot-details');box.hidden=false;box.replaceChildren(text('h3','Shoot results'),text('p','Provisional HyperIQA results — score comparison and full-shoot acceptance are deferred.','meta'));
  box.append(action('Download shoot report',()=>{const url=URL.createObjectURL(new Blob([JSON.stringify(detail,null,2)],{type:'application/json'}));const link=document.createElement('a');link.href=url;link.download=id+'-results.json';link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}));
  const wrap=document.createElement('div');wrap.className='table-wrap';const table=document.createElement('table');const head=document.createElement('thead');const hr=document.createElement('tr');for(const title of ['Photo','State','Model score','Output / issue'])hr.append(text('th',title));head.append(hr);table.append(head);const body=document.createElement('tbody');
  for(const item of detail.items){const row=document.createElement('tr');row.append(text('td',item.source.split(/[\\/]/).pop()),text('td',item.state),text('td',item.score==null?'—':item.score.toFixed(1)),text('td',item.error||item.output||'Pending'));body.append(row);}table.append(body);wrap.append(table);box.append(wrap);
 }catch(error){message(error.message);}}
 function render(result){const encoded=JSON.stringify(result);if(encoded===lastRendered)return;lastRendered=encoded;const focused=document.activeElement?.dataset.shootFocus;const box=get('shoot-jobs');box.replaceChildren();busy=result.jobs.some(({job})=>['queued','running','cancel_requested'].includes(job.state));get('shoot-start').disabled=!token||busy;
  for(const record of result.jobs){const {job}=record;const article=document.createElement('article');article.className='shoot-job';article.append(text('h3',job.source.split(/[\\/]/).filter(Boolean).pop()||'Shoot'),text('p',`${record.processed} / ${job.total} processed · ${record.completed} verified outputs · ${record.failed} errors · ${record.needsIdentification} need identification`,'meta'));
   const progress=document.createElement('progress');progress.max=job.total;progress.value=record.processed;progress.setAttribute('aria-label','Shoot progress');article.append(progress,text('p',job.state.replaceAll('_',' '),'meta'));
   const destinations=Object.entries(record.destinations).map(([folder,count])=>`${folder}: ${count}`).join(' · ');if(destinations)article.append(text('p',destinations,'meta'));if(job.error)article.append(text('p',job.error,'meta'));if(job.notification)article.append(text('p','Notification: '+job.notification,'meta'));
   const buttons=document.createElement('div');buttons.className='shoot-actions';buttons.append(action('View results',()=>showDetails(job.id),job.id+'-results'));
   if(['running','queued'].includes(job.state))buttons.append(action('Cancel after current photo',async()=>{try{await api('jobs/'+job.id+'/cancel',{});await refresh();}catch(error){message(error.message);}},job.id+'-cancel'));
   if(['paused','cancelled','completed_with_errors'].includes(job.state)&&!busy)buttons.append(action('Resume / retry unfinished',async()=>{try{await api('jobs/'+job.id+'/resume',{});await refresh();}catch(error){message(error.message);}},job.id+'-resume'));
   article.append(buttons);box.append(article);
  }if(focused){const button=[...box.querySelectorAll('button')].find(b=>b.dataset.shootFocus===focused)||[...box.querySelectorAll('button')].find(b=>b.dataset.shootFocus===focused.replace(/-(?:cancel|resume)$/,'-results'));button?.focus();}
 }
 async function refresh(){try{render(await api('jobs'));if(selected)await showDetails(selected);}catch(error){message(error.message);}}
 for(const field of ['source','output'])get('shoot-'+field+'-browse').addEventListener('click',async()=>{const button=get('shoot-'+field+'-browse');button.disabled=true;try{const chosen=await api('pick-folder',{});if(chosen.path){get('shoot-'+field).value=chosen.path;submissionId=null;}}catch(error){message(error.message);}finally{button.disabled=false;}});
 for(const field of ['shoot-source','shoot-output','shoot-kind'])get(field).addEventListener('change',()=>submissionId=null);
 get('shoot-form').addEventListener('submit',async event=>{event.preventDefault();if(!token||busy)return;get('shoot-start').disabled=true;try{const created=await api('jobs',{source:get('shoot-source').value.trim(),output:get('shoot-output').value.trim(),kind:get('shoot-kind').value,requestId:submissionId||(submissionId=crypto.randomUUID())});submissionId=null;selected=created.job.id;message('Shoot queued. You can leave this dashboard open while Rust processes it.');await refresh();}catch(error){message(error.message);get('shoot-start').disabled=busy;}});
 async function connect(){try{const config=await api('config');token=config.clientToken;if(!get('shoot-output').value)get('shoot-output').value=config.defaultOutput;get('shoot-start').disabled=busy;message('Local Rust worker ready. Select a shoot to start.');await refresh();}catch(error){message(error.message);}}
 connect();setInterval(()=>{if(document.hidden)return;if(token)refresh();else connect();},3000);
})();
