"""Compile the complete review schemas on the real backend; generate only one token, no image or grade."""
import importlib.util,json,sys
from pathlib import Path
root=Path(__file__).resolve().parents[1];spec=importlib.util.spec_from_file_location('visual',root/'analysis/visual_reviewer.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
record,key,alias=v.runtime(sys.argv[1]);schemas=json.loads((root/'analysis/vision-schemas.json').read_text())
for name,schema in schemas.items():
    result=v.api('/v1/chat/completions',key,{'model':alias,'messages':[{'role':'user','content':'Schema compiler component check only. Generate one JSON token. No photo is being graded.'}],'max_tokens':1,'temperature':0,'response_format':{'type':'json_object','schema':schema}},timeout=120)
    assert result['choices'][0]['message']['content'].lstrip().startswith('{'),result
print(json.dumps({'actualBackendSchemaCompilation':list(schemas),'realPhotoScored':False,'shootStarted':False}))
