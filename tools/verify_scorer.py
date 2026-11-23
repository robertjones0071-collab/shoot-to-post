"""Read-only photo comparison: original PyTorch, ONNX, and the native Rust runner."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time
import uuid
import numpy as np
import torch
from PIL import Image, ImageOps
from export_hyperiqa import original_model, sha, ROOT

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--image', type=Path, required=True)
    parser.add_argument('--executable', type=Path, required=True)
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--model', type=Path, default=ROOT / '.local/hyperiqa.onnx')
    args = parser.parse_args()
    before = sha(args.image)
    folder = ROOT / '.local/validation'
    folder.mkdir(parents=True, exist_ok=True)
    run_id = uuid.uuid4().hex
    tensor_file = folder / (run_id + '.f32')
    started = time.perf_counter()
    command = [str(args.executable.resolve()), '--model', str(args.model.resolve()),
               '--model-sha256', sha(args.model), '--runtime', str(args.runtime.resolve()),
               '--image', str(args.image.resolve()), '--dump-input', str(tensor_file)]
    result = subprocess.run(command, text=True, encoding='utf-8', capture_output=True, timeout=180)
    if result.returncode:
        raise RuntimeError('Rust scorer failed: ' + result.stderr[-3000:])
    rust = json.loads(result.stdout)
    rust_seconds = time.perf_counter() - started
    inputs = np.fromfile(tensor_file, dtype='<f4').reshape(10, 1, 3, 224, 224)
    torch.set_num_threads(2)
    torch.set_num_interop_threads(1)
    upstream, hyper = original_model(ROOT / '.local/hyperiqa-source', ROOT / '.local/koniq_pretrained.pkl')

    def predict(patches):
        values = []
        with torch.inference_mode():
            for patch in patches:
                params = hyper(torch.from_numpy(patch.copy()))
                values.append(float(upstream.TargetNet(params)(params['target_in_vec']).item()))
        return values

    torch_scores = predict(inputs)
    del inputs
    # A second comparison includes the original Pillow JPEG decode/bilinear resize path.
    # Fixed matching crops replace random sampling in both paths for reproducibility.
    with Image.open(args.image) as original:
        image = ImageOps.exif_transpose(original).convert('RGB').resize((384, 512), Image.Resampling.BILINEAR)
        rgb = np.asarray(image).astype(np.float32) / np.float32(255)
    rgb = (rgb - np.array([.485, .456, .406], dtype=np.float32)) / np.array([.229, .224, .225], dtype=np.float32)
    pillow_inputs = [rgb[y:y+224, x:x+224].transpose(2, 0, 1)[None] for x, y in rust['cropCoordinates']]
    pillow_scores = predict(pillow_inputs)
    rust_scores = np.array(rust['patchScores'])
    inference_error = float(np.max(np.abs(rust_scores - torch_scores)))
    pillow_mean_error = abs(float(np.mean(pillow_scores)) - rust['rawQualityScore'])
    # These tolerances are fixed before measuring actual-photo results.
    assert inference_error <= .001, ('Rust/ONNX vs original network discrepancy', inference_error)
    assert pillow_mean_error <= .1, ('End-to-end decode/resize discrepancy', pillow_mean_error)
    assert rust['rubricScore'] is None and rust['sourceUnchanged']
    assert sha(args.image) == before == rust['sourceSha256']
    assert abs(float(np.mean(torch_scores)) - rust['rawQualityScore']) <= .001
    bad = subprocess.run([*command[:4], '0' * 64, *command[5:-2]], text=True, encoding='utf-8', capture_output=True, timeout=60)
    assert bad.returncode != 0 and not bad.stdout.strip() and 'checksum mismatch' in bad.stderr
    report = {'passed': True, 'photoPath': str(args.image.resolve()), 'sourceSha256': before,
              'modelSha256': rust['modelSha256'], 'profile': rust['preprocessing'],
              'sourceUnchanged': True, 'rustScore': rust['rawQualityScore'],
              'originalNetworkScoreSameInput': float(np.mean(torch_scores)),
              'originalPillowPipelineScore': float(np.mean(pillow_scores)),
              'maxPerPatchInferenceError': inference_error, 'pillowPipelineMeanError': pillow_mean_error,
              'inferenceTolerance': .001, 'pillowMeanTolerance': .1,
              'rustWallSeconds': rust_seconds, 'checksumMismatchRejected': True,
              'limits': 'One-photo implementation equivalence; not aviation-quality or rubric calibration. Ten fixed crops replace random sampling. No photos, archive scores or model weights were written by inference.'}
    target = folder / (run_id + '.json')
    target.write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items() if k not in ('photoPath', 'sourceSha256')}), flush=True)

if __name__ == '__main__':
    main()
