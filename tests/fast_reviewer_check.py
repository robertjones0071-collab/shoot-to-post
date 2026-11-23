"""Compact reviewer contract tests; no real-photo grading or legacy-score comparison."""
import importlib.util,json,sys,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
from PIL import Image
ROOT=Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'analysis'))
import fast_visual_reviewer as v
class Checks(unittest.TestCase):
 def observation(self):return {'k':1,'c':1,'m':1,'z':[4]*5,'q':[6,6,6,5,5,5],'d':[]}
 def run_fake(self,repair=False):
  temp=tempfile.TemporaryDirectory();self.addCleanup(temp.cleanup);source=Path(temp.name)/'old_grade_99.png';Image.new('RGB',(1200,800),'gray').save(source)
  record={'modelId':'synthetic','revision':'test','modelSha256':'a','projectorSha256':'b','runtimeVersion':'test'}
  request={'source':str(source),'manifest':str(Path(temp.name)/'manifest.json'),'legacyScore':99};obs=self.observation();response={'choices':[{'finish_reason':'stop','message':{'content':json.dumps({'c':2}if repair else obs)}}]}
  if repair:request.update({'repairField':'c','previousObservation':obs,'correction':'c: invalid framing'})
  with patch.object(v.base,'runtime',return_value=(record,'key','alias')),patch.object(v.base,'api',return_value=response)as api:
   result=v.review(request)
  return result,api.call_args,obs
 def test_single_call_compact_and_no_prior_grade(self):
  result,call,obs=self.run_fake();self.assertEqual(result['compactObservation'],obs);self.assertNotIn('review',result);self.assertEqual(call.args[2]['max_tokens'],192);self.assertEqual(call.kwargs['timeout'],90);self.assertNotIn('old_grade_99',json.dumps(call.args[2]));self.assertNotIn('legacyScore',json.dumps(call.args[2]))
 def test_repair_changes_only_requested_field(self):
  result,call,obs=self.run_fake(True);self.assertEqual(result['compactObservation'],{**obs,'c':2});self.assertEqual(call.args[2]['max_tokens'],24);self.assertEqual(call.kwargs['timeout'],25);self.assertEqual(list(call.args[2]['response_format']['schema']['properties']),['c'])
 def test_class_specific_schema_forbids_not_applicable_aircraft(self):
  schema=json.loads((ROOT/'analysis/fast-vision-schema.json').read_text());aircraft=[s for s in schema['anyOf']if s['properties']['k']['enum']==[1]][0];self.assertNotIn(0,aircraft['properties']['c']['enum'])
if __name__=='__main__':unittest.main()
