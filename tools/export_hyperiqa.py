"""One-time export of the original HyperIQA checkpoint. Not used by Rust inference."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import torch

ROOT = Path(__file__).resolve().parents[1]
SOURCE_COMMIT = 'c42e7279717e7dcb693a24b891fc14a4189a45ee'
CHECKPOINT_SHA256 = 'ff9277bcc68ecc10e77d88b6d0a32825ec3c85562095542734ec6212eaaf6d81'

def sha(path):
    with open(path, 'rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()

def original_model(source, checkpoint):
    commit = subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip()
    if commit != SOURCE_COMMIT or sha(checkpoint) != CHECKPOINT_SHA256:
        raise ValueError('Unexpected upstream revision or checkpoint checksum')
    subprocess.run(['git', '-C', str(source), 'diff', '--exit-code', 'HEAD', '--', 'models.py'], check=True, capture_output=True)
    spec = importlib.util.spec_from_file_location('upstream_hyperiqa', source / 'models.py')
    upstream = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(upstream)
    # The full trained state replaces every parameter. Avoid an unrelated ImageNet download.
    prior = upstream.model_zoo.load_url
    upstream.model_zoo.load_url = lambda *_args, **_kwargs: {}
    try:
        hyper = upstream.HyperNet(16, 112, 224, 112, 56, 28, 14, 7)
    finally:
        upstream.model_zoo.load_url = prior
    hyper.load_state_dict(torch.load(checkpoint, map_location='cpu', weights_only=True), strict=True)
    hyper.eval()
    return upstream, hyper

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, default=ROOT / '.local/hyperiqa-source')
    parser.add_argument('--checkpoint', type=Path, default=ROOT / '.local/koniq_pretrained.pkl')
    parser.add_argument('--output', type=Path, default=ROOT / '.local/hyperiqa.onnx')
    args = parser.parse_args()
    torch.set_num_threads(2)
    torch.set_num_interop_threads(1)
    upstream, hyper = original_model(args.source, args.checkpoint)

    class Combined(torch.nn.Module):
        def __init__(self):
            super().__init__()
            self.hyper = hyper

        def forward(self, image):
            params = self.hyper(image)
            return upstream.TargetNet(params)(params['target_in_vec']).reshape(1)

    model = Combined().eval()
    torch.manual_seed(0)
    example = torch.rand(1, 3, 224, 224)
    with torch.inference_mode():
        reference = model(example)
        torch.onnx.export(model, example, str(args.output), input_names=['image'],
                          output_names=['quality'], opset_version=17, dynamo=False)
    import onnx
    onnx.checker.check_model(str(args.output))
    record = {
        'model': 'HyperIQA KonIQ-10k', 'sourceCommit': subprocess.check_output(
            ['git', '-C', str(args.source), 'rev-parse', 'HEAD'], text=True).strip(),
        'checkpointSha256': sha(args.checkpoint), 'onnxSha256': sha(args.output),
        'onnxBytes': args.output.stat().st_size, 'opset': 17, 'inputShape': [1, 3, 224, 224],
        'exportExampleReferenceScore': float(reference.item()), 'torchVersion': torch.__version__,
        'checkpointSource': 'https://drive.google.com/file/d/1OOUmnbvpGea0LIGpIWEbOyxfWx6UCiiE/view',
        'sourceLicense': 'MIT; separate checkpoint redistribution not established',
    }
    args.output.with_suffix('.source.json').write_text(json.dumps(record, indent=2), encoding='utf-8')
    print(json.dumps(record), flush=True)

if __name__ == '__main__':
    main()
