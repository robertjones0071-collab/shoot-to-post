# Rust scoring proof

Implemented: a Windows Rust executable that reads a JPEG or PNG, applies EXIF orientation, prepares ten fixed normalized crops, runs the original pretrained HyperIQA network through ONNX Runtime, and prints a JSON result. The inference path does not invoke Python. `ort` calls the native ONNX Runtime DLL; this is Rust application code using a native runtime, not a pure-Rust neural-network implementation.

## Verified result

Read-only comparison on one existing full-size 6000 by 3368 aviation JPEG:

| Check | Observed result |
| --- | --- |
| Rust mean quality score | 72.94557953 |
| Original PyTorch mean, identical input tensors | 72.94558029 |
| Largest individual crop difference | 0.00000763; required <= 0.001 |
| Original Pillow decode/resize pipeline, same crop coordinates | 72.95266647 |
| Full-pipeline mean difference | 0.00708694; required <= 0.1 |
| Repeated Rust score | Identical in the observed repeat |
| Repeated Rust elapsed time | 3.047 seconds, including model loading, decode, hashes and inference |
| Source bytes | Same SHA-256 before/after |
| Wrong model checksum | Rejected with nonzero exit and no score output |
| Rust unit tests | Preprocessing/normalization and invalid-result checks passed |

The first validation subprocess took about 27.5 seconds. The repeat is one warm observation on this PC, not a shoot-wide speed guarantee. The private validation report, source path, input tensors and model artifacts stay under ignored `.local/`.

This proves the implementation runs and agrees with its reference for the tested image. It does not establish aviation accuracy, agreement with historical rubric scores, CR3 support, a complete batch job or dashboard integration.

## Reproducibility profile

Source: `SSL92/hyperIQA`, commit `c42e7279717e7dcb693a24b891fc14a4189a45ee`.

Original checkpoint SHA-256: `ff9277bcc68ecc10e77d88b6d0a32825ec3c85562095542734ec6212eaaf6d81`.

Observed exported ONNX SHA-256: `09c184d0e3625649d585177c2ac81543f6e2c189d850ed76874cc013fa25fb29`.

ONNX opset 17, input `[1,3,224,224]`, CPU inference with two intra-op threads and one inter-op thread. Rust dependencies are in `Cargo.lock`; the tested runtime is ONNX Runtime 1.22.1 with `ort` 2.0.0-rc.10. Export used PyTorch 2.8.0+cpu and ONNX 1.18.0.

Profile `hyperiqa-grid10-triangle-v1` orients the image, resizes to width 384 / height 512 with the Rust image crate's triangle filter, and uses ImageNet normalization. Ten deterministic `(left,top)` crops replace the original demo's ten random crops: `(0,0)`, `(80,0)`, `(160,0)`, `(0,144)`, `(80,144)`, `(160,144)`, `(0,288)`, `(80,288)`, `(160,288)`, `(40,72)`. This is an explicit repeatability choice. Rust's decode/resize is not assumed pixel-identical to Pillow; the validation reports both network equivalence on matching tensors and the end-to-end difference with Pillow.

Outputs preserve the raw model result without clamping or presenting it as a historical rubric score. `rubricScore` is null. No destination folder is selected from this uncalibrated result.

## Build

The default path uses a standard Windows Rust/MSVC development environment:

```powershell
./tools/build-scorer.ps1 -Test
./tools/build-scorer.ps1
```

This machine was verified with the installed `stable-x86_64-pc-windows-gnu` toolchain, its bundled `rust-lld` and `dlltool`, and a portable MinGW assembler at `.local/binutils/mingw64/bin`:

```powershell
./tools/build-scorer.ps1 -Gnu -Test
./tools/build-scorer.ps1 -Gnu
```

The local assembler package was MSYS2 `mingw-w64-x86_64-binutils-2.47-5-any.pkg.tar.zst`, SHA-256 `a5e6de675bcd9d47bdb94f555ab6af9589a4c7dc813ab053c1c9c71f4aa03406`, with its zlib, zstd, gettext-runtime, libiconv and libwinpthread DLL dependencies. Artifacts were verified against the official package metadata. These are local build tools, not bundled application files. No system-wide Visual Studio installation was performed.

## Prepare the model once

Python is needed for exporting/reference verification, not for the Rust inference command. Use a separate Python environment with PyTorch 2.8.0, torchvision 0.23.0 and Pillow; install the additional pinned export packages from `tools/export-requirements.txt`. Do not change a running dashboard's environment.

1. Clone `https://github.com/SSL92/hyperIQA.git` into `.local/hyperiqa-source` and check out the exact source commit above. Retain its MIT license.
2. Obtain `koniq_pretrained.pkl` from the authors' checkpoint link in their [README](https://github.com/SSL92/hyperIQA/blob/master/README.md), and place it at `.local/koniq_pretrained.pkl`. The exporter verifies the pinned checkpoint checksum and refuses a different source revision or modified model source.
3. Run `python tools/export_hyperiqa.py`. It writes `.local/hyperiqa.onnx` and `.local/hyperiqa.source.json` and validates the ONNX graph.
4. Make the matching ONNX Runtime DLL available. For the existing `score-photo.ps1` defaults, the pinned runtime package is under `.local/export-packages/onnxruntime/capi/onnxruntime.dll`. A direct CLI invocation can specify another matching runtime path.

The source is MIT-licensed. A separate redistribution grant for the externally hosted checkpoint has not been established; weights are not committed or redistributed here. The application project itself still has no selected open-source license.

## Run locally

After the model and runtime are present:

```powershell
./tools/score-photo.ps1 -ImagePath 'D:/your-shoot/photo.jpg'
```

Or use the executable directly:

```text
shoot-to-post-scorer --model MODEL.onnx --model-sha256 EXPECTED_SHA256 --runtime onnxruntime.dll --image PHOTO.jpg
```

The optional `--dump-input NEW_FILE` diagnostic writes little-endian float32 model inputs, refuses to overwrite a file, and is used only for reference comparisons. Normal scoring writes no files. The command rejects changed sources, invalid input, mismatched model bytes, incomplete model output and non-finite scores.

For the reference comparison, run `python tools/verify_scorer.py --image PHOTO --executable EXE --runtime DLL` in the export environment. It checks the original PyTorch network on Rust-prepared inputs, the original Pillow pipeline with matching crops, source preservation, output arithmetic and wrong-checksum rejection. Its report contains local photo information and must stay out of Git.

## Still outside this proof

CR3 rendering, broader model/rubric comparison, full cull-through-review automation, batch progress/resume, file organization, notifications and the dashboard Start button. The existing dashboard and archive were not modified by this change.
