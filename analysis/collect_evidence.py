"""Bridge existing local analysis/OCR helpers; never generates rubric scores."""
import hashlib, importlib.util, json, math, sys, tempfile, sqlite3
from pathlib import Path
from PIL import Image, ImageOps

def load(path):
    spec=importlib.util.spec_from_file_location('existing_helper',path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module

def collect(request):
    if request['mode']=='identity':
        database=Path(request['database']).resolve()
        with sqlite3.connect(database.as_uri()+'?mode=ro',uri=True,timeout=2) as db:
            db.execute('PRAGMA query_only=ON')
            rows=db.execute('SELECT DISTINCT airline,variant,registration FROM files WHERE sha256=? AND present=1',(request['sourceSha256'],)).fetchall()
            candidates=[[],[],[]]
            for row in rows:
                for i,value in enumerate(row):
                    if value and value.strip().lower() not in ('unknown','unk') and value.casefold() not in [v.casefold() for v in candidates[i]]:candidates[i].append(value)
            # A saved user confirmation takes precedence over filename-derived registration.
            tables={r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
            if {'photo_candidates','registration_reviews'}<=tables:
                confirmed=[r[0] for r in db.execute('SELECT DISTINCT r.registration FROM registration_reviews r JOIN photo_candidates p ON p.photo_id=r.photo_id WHERE p.expected_sha256=?',(request['sourceSha256'],)) if r[0]]
                if confirmed:candidates[2]=confirmed
        db.close()
        return {'basis':'Exact SHA-256 archive metadata; saved user registration confirmations preferred; not new visual identification','matchCount':len(rows),'labels':[v[0] if len(v)==1 else 'Unknown' for v in candidates],'conflicts':[len(v)>1 for v in candidates],'candidates':candidates}
    source=Path(request['source']); helper=load(request['helper'])
    before=hashlib.sha256(source.read_bytes()).hexdigest()
    with Image.open(source) as raw:
        width,height=ImageOps.exif_transpose(raw).size
        if width*height>100_000_000:raise ValueError('Image exceeds 100 megapixels')
        metadata={}
        exif=raw.getexif()
        try:exif.update(exif.get_ifd(34665))
        except (KeyError,ValueError):pass
        for key,name in [(271,'make'),(272,'camera'),(36867,'captureDate'),(33434,'exposureSeconds'),(33437,'aperture'),(34855,'iso'),(37386,'focalLength')]:
            value=exif.get(key)
            if value is not None:
                metadata[name]=float(value) if isinstance(value,(int,float)) or hasattr(value,'numerator') else str(value)[:200]
    if request['mode']=='technical':
        # Tile the full frame at native resolution, not purported aircraft zones.
        nx=max(1,math.ceil(width/3500));ny=max(1,math.ceil(height/3500));tiles=[]
        for y in range(ny):
            for x in range(nx):
                roi={'x':x/nx,'y':y/ny,'width':1/nx,'height':1/ny}
                if width/nx<64 or height/ny<64:raise ValueError('Image too small for native measurements')
                tiles.append(helper.analyze(source,roi,'AMB','uncertain'))
        result={'engine':'existing-opencv-full-frame-tiles-v1','dimensions':[width,height],'metadata':metadata,'tiles':tiles,
                'limitations':['Full-frame tiles include background and are not detected aircraft zones.','No focus/motion grade, clipping severity, white-balance judgment or subject completeness is inferred from these metrics.']}
    elif request['mode']=='ocr':
        # Existing ROI OCR is reused on one bounded full-frame preview. Suggestions only.
        with Image.open(source) as raw:
            image=ImageOps.exif_transpose(raw).convert('RGB');image.thumbnail((2000,2000),Image.Resampling.LANCZOS)
            with tempfile.TemporaryDirectory(prefix='shoot-ocr-') as directory:
                preview=Path(directory)/'preview.png';image.save(preview)
                result=helper.analyze(preview,{'x':0,'y':0,'width':1,'height':1})
                result['previewDimensions']=list(image.size)
        result.pop('sourceSha256',None)
        result['sourceDimensions']=[width,height]
        result['scope']='Full-frame bounded preview; polygons normalized to oriented frame'
        result['limitations']+=['No text is automatically accepted as a registration or aircraft identity.','Small registration text may require a selected native-resolution region.']
    else:raise ValueError('Unknown evidence mode')
    if hashlib.sha256(source.read_bytes()).hexdigest()!=before:raise RuntimeError('Input changed during evidence collection')
    result['inputSha256']=before
    result['helperSha256']=hashlib.sha256(Path(request['helper']).read_bytes()).hexdigest()
    return result

if __name__=='__main__':
    try:
        sys.stdin.reconfigure(encoding='utf-8');sys.stdout.reconfigure(encoding='utf-8')
        print(json.dumps(collect(json.load(sys.stdin)),allow_nan=False))
    except Exception as error:
        print(json.dumps({'error':str(error)}));sys.exit(2)
