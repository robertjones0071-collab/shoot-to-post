# Automatic local visual reviewer

The Processing tab defaults to **Detect automatically**. Select a shoot directory and press Start Cull Through Review. No manual category scores are required in the normal workflow. Starting the dashboard alone does not process photos.

## Model and runtime

- Model: [Qwen/Qwen3-VL-2B-Instruct-GGUF](https://huggingface.co/Qwen/Qwen3-VL-2B-Instruct-GGUF), Apache-2.0, pinned revision `52d6c8ffea26cc873ac5ad116f8631268d7eb503`.
- Language weights: official Q8_0 GGUF, SHA-256 `1e8db19207c8ce0733ddd78c2eff8a9e22c27c82f4443df94c25792ed8fe04f2`.
- Vision projector: official Q8_0 GGUF, SHA-256 `f9a68fabba69c3b81e153367b2c7521030b0fa8bb0de400c9599c8e6725f9c82`.
- Runtime: official [llama.cpp b9000](https://github.com/ggml-org/llama.cpp/releases/tag/b9000) Windows CPU distribution, MIT. The faster runtime and encoder were selected using isolated generated-image benchmarks. ZIP SHA-256 `8294e287933d3212aa93a32e1ceb800722bede14d9692d405679d4ba77cf05db`.

Run `tools/setup-vision.ps1` for the pinned downloads and checksums. It retains local license texts and writes a private asset manifest under `.local/vision`. Models, binaries and the private API key are not committed. The adapter supplies these locations to the Rust worker.

## Automatic path

1. Reuse the existing native OpenCV measurements, OCR suggestions and read-only exact-byte archive identity evidence.
2. Render CR3 temporarily at full resolution when needed. Original JPEG/CR3 bytes remain unchanged.
3. Qwen inspects an overview to classify the primary subject and ground five inspection zones.
4. Extract bounded 512-pixel native crops around the proposed zone centres, coalescing identical rectangles while preserving zone labels. Submit the original-frame overview plus these labelled crops for the six rubric categories, motion, completeness, evidence, defects and identification hypotheses. A crop is never used as evidence that the original image boundary truncates the aircraft. Incorrect grounding remains a model risk.
5. Schema-constrained output is parsed into the Rust review type. Rust applies rubric 14 dependencies and ceilings, then naming/routing uses the final grade. One automatic correction retry is allowed after deterministic validation failure. No old grades, filenames or HyperIQA diagnostic values are supplied to the model.
6. Validated assessments proceed directly to checksum-verified copies and the existing summary/notification handler. Mixed shoots route per-photo detected class. Unassessable images and invalid responses remain Unrated; runtime failures are reported and can be retried. A manual override is optional, not part of the normal path.

Model-derived identity remains labelled as a hypothesis. Existing/automatic conflicts stay Unknown. An automatically proposed registration additionally needs exact corroboration from a registration-like OCR line. This agreement is not proof of correctness. Unknown identity does not prevent a supported technical grade.

The report stores model and projector hashes, revision, runtime version, prompt/schema hashes, original/rendered checksums, crop boxes, detected subject, token usage, model observations, validation results and final rubric arithmetic. Automatic confidence is at most Medium until calibration; this widens uncertainty without imposing a confidence-based score cap.

## Local process lifecycle

Inference uses four CPU threads and one local model slot. The authenticated llama server binds only to `127.0.0.1:47835`; images are sent only to that loopback address. HTTP proxy environment variables are disabled for these requests. No model tools, cloud API or public UI are enabled. The private key stays in an ignored file, not command-line arguments or browser responses.

The reviewer starts a hidden supervisor lazily on first use. It holds the model across photos and terminates its own server after three minutes without a request heartbeat, releasing RAM. Long CPU requests refresh that heartbeat. Runtime/model hashes are checked before launch. A later run starts it again automatically. A first model load can be slow; CPU inspection can take minutes per photo. No completed-shoot throughput estimate has been established.

## Verification boundary

Component checks use synthetic fixtures and generated images. They exercise crop bounds, two-pass orchestration, exclusion of old grades, typed automatic assessment validation, rubric rules and output safety. The real-model generated-color-card smoke check passed: it identified a red rectangle and returned constrained JSON. Both full output schemas also compiled on the actual backend using one-token checks. No collection photo was graded and no shoot was started. The selected profile took about 21 seconds on the generated-card test, versus 164 seconds in a fresh baseline run. This is not photo/shoot throughput; see [benchmark details and limitations](REVIEWER-PERFORMANCE.md).

Score comparisons, calibration against historical reviews, and the full-shoot acceptance test remain explicitly deferred. Automated assessment accuracy, complete batch recovery and actual notification delivery have not been established by these component checks. Open-source model inference is not proof that every grade or identity is correct.

The active dynamic profile uses 256-1024 image tokens, preserving individual native detail crops without the old unnecessary enlargement. Grounding locations are cached by source/model/runtime/prompt/schema/profile for correction retries; category scores are not cached. Runtime settings are recorded in each report.
