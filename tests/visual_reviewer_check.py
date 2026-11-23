"""Fast component tests: crop integrity and two-pass orchestration with mocked model responses."""
import copy,hashlib,importlib.util,json,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
from PIL import Image
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('visual',ROOT/'analysis/visual_reviewer.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
class Checks(unittest.TestCase):
 def test_native_crop_bounds(self):
  im=Image.new('RGB',(1200,800));views=v.crop_views(im,{'zoneCenters':{'cockpit':[1000,1000],'tail':[-1,-1]}})
  self.assertEqual(views[0][1],[688,288,1200,800]);self.assertEqual(views[0][2].size,(512,512))
  with self.assertRaises(ValueError):v.crop_views(im,{'zoneCenters':{'nose':[1001,10]}})
 def test_two_pass_automatic_without_historical_grades(self):
  record={'modelId':'synthetic-contract-model','revision':'synthetic','modelSha256':'a'*64,'projectorSha256':'b'*64,'runtimeVersion':'synthetic'}
  ground={'subjectClass':'aircraft','description':'synthetic primary subject','zoneCenters':{k:[500,500] for k in ['cockpit','nose','midbody','mechanical','tail']}}
  review=json.loads((ROOT/'examples/review-submission.json').read_text())['review']
  review['confidence']='high';review['uncertainty']=1;review['motionMode']='AMB';review['motionConfidence']='high'
  answer={'assessable':True,'reason':'Synthetic contract only','review':review,'identification':{'airline':'Unknown','aircraft':'Unknown','registration':'Unknown','evidence':{'airline':'Not established','aircraft':'Not established','registration':'Not established'}}}
  calls=[]
  def chat(key,alias,messages,schema,max_tokens):
   calls.append(messages)
   return (copy.deepcopy(ground if len(calls)==1 else answer),{})
  with tempfile.TemporaryDirectory() as temp:
   source=Path(temp)/'DO_NOT_LEAK_OLD_SCORE_100.png';Image.new('RGB',(1200,800),'gray').save(source);before=v.digest(source)
   with patch.object(v,'runtime',return_value=(record,'private-test-key','model')),patch.object(v,'chat',side_effect=chat):result=v.review({'source':str(source),'manifest':str(Path(temp)/'manifest.json'),'legacyScore':100,'filename':'OLD_SCORE_100'})
   self.assertEqual(len(calls),2);self.assertNotIn('DO_NOT_LEAK_OLD_SCORE',json.dumps(calls));self.assertNotIn('legacyScore',json.dumps(calls))
   self.assertEqual(result['effectiveKind'],'aviation');self.assertEqual(result['review']['confidence'],'medium');self.assertGreaterEqual(result['review']['uncertainty'],3);self.assertEqual(result['review']['motionConfidence'],'low');self.assertEqual(result['inputSha256'],before);self.assertEqual(v.digest(source),before);self.assertEqual(len(result['viewContext']['nativeCrops']),5);self.assertEqual(result['uniqueDetailImages'],1);self.assertEqual(sum(p['type']=='image_url' for p in calls[1][1]['content']),2)
 def test_distinct_native_rectangles_are_preserved(self):
  im=Image.new('RGB',(1600,800));views=v.crop_views(im,{'zoneCenters':{'cockpit':[300,500],'nose':[100,500],'midbody':[500,500],'mechanical':[700,500],'tail':[900,500]}})
  unique=v.unique_views(views);self.assertEqual(len(unique),5)
  for names,box,crop in unique:self.assertEqual(crop.tobytes(),im.crop(box).tobytes())
 def test_grounding_cache_invalidates_on_model_or_source_change(self):
  with tempfile.TemporaryDirectory() as temp:
   manifest=Path(temp)/'manifest.json';record={'modelSha256':'a','projectorSha256':'b','runtimeVersion':'c'}
   path=v.grounding_cache(manifest,'source',record,'prompt',{})
   ground={'subjectClass':'aircraft','description':'Synthetic location','zoneCenters':{k:[500,500] for k in ['cockpit','nose','midbody','mechanical','tail']}}
   v.save_grounding(path,ground);self.assertEqual(v.read_grounding(path),ground)
   self.assertNotEqual(path,v.grounding_cache(manifest,'changed',record,'prompt',{}));self.assertNotEqual(path,v.grounding_cache(manifest,'source',{**record,'projectorSha256':'changed'},'prompt',{}));self.assertNotEqual(path,v.grounding_cache(manifest,'source',record,'changed',{}))
 def test_contact_sheets_keep_every_native_pixel_and_zone(self):
  views=[]
  for i in range(5):
   im=Image.new('RGB',(512,512),(i*40,20,30));views.append(([str(i)],[i*10,0,i*10+512,512],im))
  sheets=v.detail_sheets(views);self.assertEqual(len(sheets),2);self.assertEqual(sheets[0][1].size,(1024,1024));self.assertEqual(sheets[1][1].size,(512,512))
  recovered=[]
  for panels,sheet in sheets:
   for panel in panels:
    index=int(panel['zones'][0]);self.assertEqual(sheet.crop(panel['sheetPixelBox']).tobytes(),views[index][2].tobytes());recovered.append(index)
  self.assertEqual(recovered,list(range(5)))
 def test_runtime_alias_changes_with_encoder_or_threads(self):
  record={'modelSha256':'a'*64,'projectorSha256':'b'*64,'cpuThreads':4}
  self.assertNotEqual(v.model_alias(record),v.model_alias({**record,'projectorSha256':'c'*64}));self.assertNotEqual(v.model_alias(record),v.model_alias({**record,'cpuThreads':6}))
 def test_bad_cached_locations_are_recomputed(self):
  with tempfile.TemporaryDirectory() as temp:
   path=Path(temp)/'cached.json';path.write_text('{bad json');self.assertIsNone(v.read_grounding(path))
   v.save_grounding(path,{'subjectClass':'aircraft'});self.assertIsNone(v.read_grounding(path))
 def test_dynamic_profile_keeps_native_crops_individual(self):
  self.assertEqual(v.view_profile({'detailLayout':'individual'}),'overview1280-native512-dynamic-v4')
 def test_retry_reuses_locations_without_reusing_grades(self):
  record={'modelId':'synthetic','revision':'test','modelSha256':'a','projectorSha256':'b','runtimeVersion':'test','detailLayout':'individual'}
  ground={'subjectClass':'aircraft','description':'synthetic','zoneCenters':{k:[500,500] for k in ['cockpit','nose','midbody','mechanical','tail']}}
  answer={'assessable':False,'reason':'Synthetic insufficient evidence','review':{},'identification':{}}
  calls=[]
  def chat(*args):
   calls.append(args);return(copy.deepcopy(ground if len(calls)==1 else answer),{})
  with tempfile.TemporaryDirectory() as temp:
   source=Path(temp)/'fixture.png';Image.new('RGB',(1200,800),'gray').save(source)
   request={'source':str(source),'manifest':str(Path(temp)/'manifest.json')}
   with patch.object(v,'runtime',return_value=(record,'key','alias')),patch.object(v,'chat',side_effect=chat):
    first=v.review(request);second=v.review({**request,'correction':'Synthetic rule rejection'})
   self.assertEqual(len(calls),3);self.assertFalse(first['groundingCacheHit']);self.assertTrue(second['groundingCacheHit']);self.assertEqual(len(second['viewContext']['nativeCrops']),5);self.assertEqual(second['encodedDetailSheets'],1);self.assertFalse(second['assessable'])
 def test_uncertain_subject_never_makes_a_grade(self):
  with tempfile.TemporaryDirectory() as temp:
   source=Path(temp)/'blank.png';Image.new('RGB',(128,128),'white').save(source)
   with patch.object(v,'runtime',return_value=({'modelId':'synthetic'},'key','alias')),patch.object(v,'chat',return_value=({'subjectClass':'uncertain'},{})) as chat:
    result=v.review({'source':str(source),'manifest':str(Path(temp)/'manifest.json')});self.assertFalse(result['assessable']);self.assertNotIn('review',result);self.assertEqual(chat.call_count,1)
if __name__=='__main__':unittest.main()
