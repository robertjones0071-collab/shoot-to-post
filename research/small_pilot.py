"""Bounded numerical feasibility experiment, not an app reviewer or calibrated rubric scorer."""
import argparse,collections,hashlib,importlib.util,json,math,random,sys,time
from pathlib import Path
import numpy as np
from PIL import Image,ImageOps
p=argparse.ArgumentParser();p.add_argument('workspace',type=Path);p.add_argument('output',type=Path);args=p.parse_args();workspace=args.workspace.resolve();root=workspace/'shoot-to-post';out=args.output.resolve();out.mkdir(parents=True,exist_ok=True)
labels=json.loads((out/'labels.private.json').read_text());rng=random.Random(20261122)
verified={'moved_verified','renamed_verified','unchanged_verified','renamed_copy_created'}
eligible=[r for r in labels if r['hasCategoryBreakdown']and sum(r['categories'])==r['score']and r.get('labelOutcome')in verified and Path(r['path']).suffix.lower()in('.jpg','.jpeg')and r['day']]
days=sorted({r['day']for r in eligible});assert len(days)>=4,'Insufficient capture-day groups';days=days[-4:];holdout=days[-1]
def hamming(a,b):return (int(a,16)^int(b,16)).bit_count() if a and b and len(a)==len(b)else 10**6
def near(a,b):return a['sha256']==b['sha256']or(hamming(a['phash'],b['phash'])<=16 and hamming(a['dhash'],b['dhash'])<=8)
def burst(r):
 stamp=r.get('captureStamp','')
 try:return stamp[:14]+str(int(stamp[14:16])//5)
 except (ValueError,IndexError):return r['sha256']
# Held-out day is fixed before image features are computed. Different days never split a burst.
selected=[];rejected_near=0
for day in days:
 rows=[r for r in eligible if r['day']==day];rng.shuffle(rows);buckets=[[],[],[],[]]
 for r in rows:buckets[0 if r['score']<80 else 1 if r['score']<85 else 2 if r['score']<90 else 3].append(r)
 pool=[]
 while any(buckets):
  for bucket in buckets:
   if bucket:pool.append(bucket.pop())
 chosen=[];bursts=set()
 for r in pool:
  if burst(r)in bursts:continue
  if any(near(r,s)for s in selected+chosen):rejected_near+=1;continue
  if not Path(r['path']).is_file():continue
  chosen.append(r);bursts.add(burst(r))
  if len(chosen)==9:break
 if len(chosen)<4:raise RuntimeError('Not enough independent images for a day: '+day)
 selected+=chosen
train_indices=[i for i,r in enumerate(selected)if r['day']!=holdout];test_indices=[i for i,r in enumerate(selected)if r['day']==holdout]
assert set(selected[i]['day']for i in train_indices).isdisjoint({holdout})
assert all(not near(selected[i],selected[j])for i in train_indices for j in test_indices)
plan={'seed':20261122,'selection':'up to nine JPEGs per capture day; score-band round robin; one per five-minute burst; conservative perceptual near-duplicate exclusion','selected':len(selected),'training':len(train_indices),'heldOut':len(test_indices),'captureDays':days,'heldOutDay':holdout,'rejectedNearDuplicates':rejected_near,'minimumIndependentSamplesPerDay':4,'note':'Small day/burst coverage determined sample count before feature extraction; no metric-based sample tuning','ridgeAlpha':10.0,'featuresExclude':['filename','airline','aircraft','registration','captureDate','old score','old categories','notes'],'labelStatus':'historical, unversioned, unadjudicated labels; reproduction experiment only'}
(out/'pilot-plan.json').write_text(json.dumps(plan,indent=2));print(json.dumps({'pilotPlan':plan}),flush=True)
sys.path.insert(0,str(root/'.local/export-packages'));import onnxruntime as ort
spec=importlib.util.spec_from_file_location('technical',workspace/'aviation-dashboard/analysis/technical_worker.py');technical=importlib.util.module_from_spec(spec);spec.loader.exec_module(technical)
def sha(path):
 with path.open('rb')as f:return hashlib.file_digest(f,'sha256').hexdigest()
model=root/'.local/hyperiqa.onnx';model_info=json.loads((root/'.local/hyperiqa.source.json').read_text());assert sha(model)==model_info['onnxSha256']
started=time.perf_counter();options=ort.SessionOptions();options.intra_op_num_threads=2;options.inter_op_num_threads=1;session=ort.InferenceSession(str(model),sess_options=options,providers=['CPUExecutionProvider']);startup=time.perf_counter()-started;input_name=session.get_inputs()[0].name
CROPS=[(0,0),(80,0),(160,0),(0,144),(80,144),(160,144),(0,288),(80,288),(160,288),(40,72)]
mean=np.array([.485,.456,.406],dtype=np.float32);std=np.array([.229,.224,.225],dtype=np.float32)
feature_names=['rawHyperIQA','logMeanNativeEdgeVariance','logMinNativeEdgeVariance','logMeanNativeGradient','overviewHighlightClipPct','overviewShadowClipPct','overviewAnyChannelClipPct','overviewNoiseProxy','noiseProxyMissing','logMegapixels']
results=[]
for i,row in enumerate(selected):
 tick=time.perf_counter();path=Path(row['path']);before=sha(path);assert before==row['sha256'],'Input hash differs from recorded label; stop rather than mislabel';decode_started=time.perf_counter()
 with Image.open(path)as raw:image=ImageOps.exif_transpose(raw).convert('RGB')
 width,height=image.size;decode=time.perf_counter()-decode_started
 tick_model=time.perf_counter();resized=np.asarray(image.resize((384,512),Image.Resampling.BILINEAR),dtype=np.float32)/np.float32(255);normalized=(resized-mean)/std;patches=[]
 for left,top in CROPS:
  patch=np.ascontiguousarray(normalized[top:top+224,left:left+224].transpose(2,0,1)[None]);patches.append(float(session.run(None,{input_name:patch})[0].reshape(-1)[0]))
 quality=float(np.mean(patches));assert math.isfinite(quality);model_seconds=time.perf_counter()-tick_model
 tick_measure=time.perf_counter();overview=image.copy();overview.thumbnail((1024,1024),Image.Resampling.LANCZOS);measure=technical.measure_rgb(np.asarray(overview));native=[]
 for frac in [.2,.5,.8]:
  w=min(512,width);h=min(512,height);left=max(0,min(width-w,round(width*frac)-w//2));top=max(0,min(height-h,height//2-h//2));native.append(technical.measure_rgb(np.asarray(image.crop((left,top,left+w,top+h)))))
 native_edge=[r['edgeVariance']for r in native];native_grad=[r['gradientEnergy']for r in native];noise=measure['noiseProxy8bit']
 values=[quality,math.log1p(float(np.mean(native_edge))),math.log1p(min(native_edge)),math.log1p(float(np.mean(native_grad))),measure['highlightClippedPct'],measure['shadowClippedPct'],measure['anyChannelClippedPct'],float('nan')if noise is None else noise,float(noise is None),math.log1p(width*height/1e6)]
 measurement_seconds=time.perf_counter()-tick_measure;assert sha(path)==before,'Source changed during pilot'
 result={**row,'features':[None if not math.isfinite(v)else v for v in values],'dimensions':[width,height],'timing':{'decodeSeconds':decode,'modelSeconds':model_seconds,'measurementSeconds':measurement_seconds,'totalFeatureSeconds':time.perf_counter()-tick},'sourceVerifiedUnchanged':True}
 results.append(result);(out/'pilot-features.private.json').write_text(json.dumps(results,indent=2));print(json.dumps({'processed':i+1,'of':len(selected),'featureSeconds':round(result['timing']['totalFeatureSeconds'],3)}),flush=True)
X=np.array([[np.nan if v is None else v for v in row['features']]for row in results],dtype=float);Y=np.array([r['score']for r in results],dtype=float);C=np.array([r['categories']for r in results],dtype=float);tr=np.array(train_indices);te=np.array(test_indices)
# Imputation, scaling and fitting use training rows only. No hyperparameter search on held-out labels.
medians=np.nanmedian(X[tr],axis=0);medians=np.where(np.isfinite(medians),medians,0);X=np.where(np.isfinite(X),X,medians);mu=X[tr].mean(axis=0);scale=X[tr].std(axis=0);scale=np.where(scale>1e-8,scale,1);Z=(X-mu)/scale
alpha=10.0
def fit_predict(columns,target):
 train=Z[tr][:,columns];test=Z[te][:,columns];intercept=target[tr].mean(axis=0);beta=np.linalg.solve(train.T@train+alpha*np.eye(train.shape[1]),train.T@(target[tr]-intercept));return test@beta+intercept,beta,intercept
mean_prediction=np.full(len(te),Y[tr].mean());quality_prediction,_,_=fit_predict([0],Y);all_prediction,beta,intercept=fit_predict(list(range(X.shape[1])),Y);category_prediction,_,_=fit_predict(list(range(X.shape[1])),C)
all_prediction=np.clip(all_prediction,0,100);quality_prediction=np.clip(quality_prediction,0,100);category_prediction=np.clip(category_prediction,0,[30,25,15,10,10,10])
def metrics(pred):
 error=pred-Y[te];return {'meanAbsoluteRecordedLabelError':float(np.abs(error).mean()),'rootMeanSquareRecordedLabelError':float(np.sqrt((error**2).mean())),'within3RecordedPoints':float(np.mean(np.abs(error)<=3))}
times=np.array([r['timing']['totalFeatureSeconds']for r in results]);max_cross_similarity=max((256-hamming(results[i]['phash'],results[j]['phash'])for i in tr for j in te if results[i]['phash']and results[j]['phash']),default=None)
summary={'plan':plan,'processedImages':len(results),'verifiedUnchanged':all(r['sourceVerifiedUnchanged']for r in results),'modelStartupSeconds':startup,'featureSeconds':{'median':float(np.median(times)),'p95':float(np.percentile(times,95)),'max':float(max(times))},'under30SecondsAllFeatureExtractions':bool(np.all(times<30)),'baselineTrainingMean':metrics(mean_prediction),'hyperIqaOnlyRidge':metrics(quality_prediction),'hyperIqaPlusMeasurementsRidge':metrics(all_prediction),'sixCategoryRidgeSum':metrics(category_prediction.sum(axis=1)),'featureNames':feature_names,'nativeSamples':'three generic native 512-pixel patches, not detected aircraft zones','hyperiqaProfile':'ten fixed grid crops; Pillow bilinear 384x512; ImageNet normalization; CPU two threads','modelSha256':model_info['onnxSha256'],'categoryTrainingTargets':'recorded historical category points; no assumed current rubric version','limits':['This pilot tests numerical speed and reproduction of weak historical labels, NOT photography-quality accuracy.','Only four capture days and a small held-out day; no confidence interval or generalization claim.','Date grouping and perceptual filtering reduce leakage; no independent scene/aircraft adjudication was performed.','No aircraft-completeness detector, motion classifier, hard-gate validation, OCR or output copying is included in the latency.','No app integration, full-shoot run, archive re-rating or historical score changes were performed.','HyperIQA is general image quality; full-frame/native sample metrics contain background and cannot by themselves diagnose subject focus.']}
(out/'pilot.json').write_text(json.dumps(summary,indent=2));(out/'pilot-predictions.private.json').write_text(json.dumps([{'sha256':results[j]['sha256'],'recordedLabel':Y[j],'meanBaseline':mean_prediction[k],'numericPilot':all_prediction[k]}for k,j in enumerate(te)],indent=2));print(json.dumps({'pilotSummary':summary}),flush=True)
