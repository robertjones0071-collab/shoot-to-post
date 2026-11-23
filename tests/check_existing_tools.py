"""Component check of the helper bridge using a generated registration card, no shoot job."""
import argparse,hashlib,json,subprocess,tempfile,sqlite3
from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
p=argparse.ArgumentParser();p.add_argument('dashboard',type=Path);args=p.parse_args();dashboard=args.dashboard.resolve();root=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='shoot-component-') as temp:
    source=Path(temp)/'synthetic.png';im=Image.new('RGB',(700,200),'white');ImageDraw.Draw(im).text((30,35),'N123AB',font=ImageFont.truetype('C:/Windows/Fonts/arial.ttf',100),fill='black');im.save(source)
    before=hashlib.sha256(source.read_bytes()).hexdigest()
    for mode,env,worker in [('technical','analysis-env','technical_worker.py'),('ocr','ocr-env','registration_worker.py')]:
        response=subprocess.run([str(dashboard/'vendor'/env/'Scripts/python.exe'),str(root/'analysis/collect_evidence.py')],input=json.dumps({'mode':mode,'source':str(source),'helper':str(dashboard/'analysis'/worker)}),text=True,encoding='utf-8',capture_output=True,timeout=90,check=True)
        value=json.loads(response.stdout);assert value['inputSha256']==before
        if mode=='technical':assert len(value['tiles'])==1 and value['tiles'][0]['score'] is None
        else:assert any(line['normalized']=='N123AB' for line in value['lines'])
        assert hashlib.sha256(source.read_bytes()).hexdigest()==before
    dbpath=Path(temp)/'archive.sqlite'
    with sqlite3.connect(dbpath) as db:
        db.executescript("CREATE TABLE files(airline TEXT,variant TEXT,registration TEXT,sha256 TEXT,present INTEGER);INSERT INTO files VALUES('Example','A320','N123AB','exact',1),('Example','A320','N456CD','exact',1);")
    db.close()
    before=hashlib.sha256(dbpath.read_bytes()).hexdigest()
    result=subprocess.run([str(dashboard/'vendor/analysis-env/Scripts/python.exe'),str(root/'analysis/collect_evidence.py')],input=json.dumps({'mode':'identity','database':str(dbpath),'sourceSha256':'exact'}),text=True,encoding='utf-8',capture_output=True,check=True)
    identity=json.loads(result.stdout);assert identity['labels']==['Example','A320','Unknown'] and identity['conflicts']==[False,False,True]
    assert hashlib.sha256(dbpath.read_bytes()).hexdigest()==before
print(json.dumps({'exactIdentityConflictsChecked':True,'syntheticOnly' :True,'existingTechnicalConnected':True,'existingOcrConnected':True,'sourceUnchanged':True,'scoreAssigned':False}))
