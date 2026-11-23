"""Actual local vision inference on a generated color card; never grades a real photo or runs a shoot."""
import importlib.util,json,sys
from pathlib import Path
from PIL import Image,ImageDraw
root=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('visual',root/'analysis/visual_reviewer.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
manifest=Path(sys.argv[1]);record,key,alias=module.runtime(manifest)
image=Image.new('RGB',(256,256),'white');ImageDraw.Draw(image).rectangle((32,32,224,224),fill='red')
schema={'type':'object','properties':{'color':{'type':'string','enum':['red','blue','green']},'shape':{'type':'string','enum':['rectangle','circle']}},'required':['color','shape'],'additionalProperties':False}
result,usage=module.chat(key,alias,[{'role':'user','content':[{'type':'text','text':'Identify the color and shape of the large colored object on the white background.'},module.image_part(image)]}],schema,80)
assert result=={'color':'red','shape':'rectangle'},result
print(json.dumps({'actualModelImageInference':True,'syntheticOnly':True,'modelId':record['modelId'],'runtime':record['runtimeVersion'],'result':result,'usage':usage,'realPhotoScored':False,'shootStarted':False}))
