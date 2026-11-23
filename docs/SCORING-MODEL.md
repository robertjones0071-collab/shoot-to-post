# V1 scoring: reuse a pretrained model

Robert's direction: the existing workflow is the foundation; reuse an open-source scoring model for v1 and run scoring from Rust. Custom training on the scored collection is deferred, not a prerequisite for the first build.

## First candidate: HyperIQA

- Original implementation: https://github.com/SSL92/hyperIQA
- Source license: MIT, copyright SSL92; retain its license when reusing source.
- The authors link a pretrained KonIQ-10k checkpoint and describe higher-is-better scores from 0 to 100: https://github.com/SSL92/hyperIQA/blob/master/README.md
- The official demo resizes to 512 by 384, evaluates ten random 224-pixel crops with ImageNet normalization, and averages their scores: https://github.com/SSL92/hyperIQA/blob/master/demo.py
- Proposed Rust runtime: `ort`, the MIT/Apache-2.0 Rust interface to ONNX Runtime: https://github.com/pykeio/ort

The first export and native Windows Rust scoring proof now work. One actual JPEG comparison passed against the original network, and a repeat returned the same result. See [the measured proof and its limits](SCORER-PROOF.md). Aviation accuracy and agreement with the historical rubric are not established. Check the checkpoint's terms/provenance separately before redistributing weights; the repository's source license alone is not a verified license for every separately hosted artifact. Keep downloaded weights outside Git and record exact origin, version and checksum.

## First build boundary

1. Obtain the original model and record a reference result without training or changing archive scores.
2. Verify that the model can be represented in ONNX and run with Rust `ort`, preserving preprocessing and outputs against the reference. Use matching fixed crop coordinates to test equivalence; otherwise the demo's random crops confound repeatability.
3. Score a small selection of existing reviewed JPEGs and measure CPU time/memory. Include pans, sharp aircraft, visibly soft frames, night scenes and boundary-truncated aircraft. Use the existing scores as comparison evidence, not a request to rescore the entire archive.
4. Expose the Rust scorer to the existing local dashboard after the result format is verified. JPEG input establishes the scorer; CR3 full-resolution rendering remains required in the integrated workflow.

## Score meaning

HyperIQA predicts general perceptual image quality. A numeric result such as 92 is not automatically the same as Robert's rubric score of 92. Preserve raw model output separately from historical scores. Measure agreement and any justified mapping before using existing routing thresholds. The model is not established as an aircraft completeness detector, registration reader, or generator of the six rubric category scores. Do not invent those outputs from a single quality number.

The official resized/random-patch approach is not full-resolution aircraft-zone inspection. Check that limitation on actual aviation examples before calling it equivalent to the existing review procedure. Reuse the current identification, technical measurements, naming and routing work where it is already implemented; record any remaining automation gaps rather than silently changing the intended workflow.

## Alternatives inspected

Google's original MUSIQ implementation publishes several quality checkpoints and is another candidate: https://github.com/google-research/google-research/tree/master/musiq . It has not been selected or converted here.

Do not assume that the current PyIQA distribution is permissively licensed: its README currently lists PolyForm Noncommercial and the NTU S-Lab license for applicable components: https://github.com/chaofengc/IQA-PyTorch . A third-party ONNX upload's license badge does not by itself establish permission for its upstream code and weights.

## Scope

Implemented scope: the one-photo Rust scoring proof, one-time model export and reference comparison. No custom training, cloud integration, dashboard redesign or archive-wide rescoring. Model downloads and private validation data stay local and ignored by Git.
