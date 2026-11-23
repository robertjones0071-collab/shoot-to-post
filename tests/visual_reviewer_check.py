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
   with patch.object(v,'runtime',return_value=(record,'private-test-key','model')),patch.object(v,'chat',side_effect=chat):result=v.review({'source':str(source),'manifest':'unused','legacyScore':100,'filename':'OLD_SCORE_100'})
   self.assertEqual(len(calls),2);self.assertNotIn('DO_NOT_LEAK_OLD_SCORE',json.dumps(calls));self.assertNotIn('legacyScore',json.dumps(calls))
   self.assertEqual(result['effectiveKind'],'aviation');self.assertEqual(result['review']['confidence'],'medium');self.assertGreaterEqual(result['review']['uncertainty'],3);self.assertEqual(result['review']['motionConfidence'],'low');self.assertEqual(result['inputSha256'],before);self.assertEqual(v.digest(source),before);self.assertEqual(len(result['viewContext']['nativeCrops']),5)
 def test_uncertain_subject_never_makes_a_grade(self):
  with tempfile.TemporaryDirectory() as temp:
   source=Path(temp)/'blank.png';Image.new('RGB',(128,128),'white').save(source)
   with patch.object(v,'runtime',return_value=({'modelId':'synthetic'},'key','alias')),patch.object(v,'chat',return_value=({'subjectClass':'uncertain'},{})) as chat:
    result=v.review({'source':str(source),'manifest':'unused'});self.assertFalse(result['assessable']);self.assertNotIn('review',result);self.assertEqual(chat.call_count,1)
if __name__=='__main__':unittest.main()
