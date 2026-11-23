# Shoot to Post

A local aviation photography dashboard, with a Windows Rust workflow for one-button shoot review.

**Workflow: Open the local dashboard → Select a shoot → Cull Through Review → Receive a local notification and summary.**

The implementation supports JPEG and Canon CR3 RAW inputs, technical evaluation using the existing rubric, evidence-backed identification, safe naming, verified output copies and a shoot summary. Original input files remain unchanged in the existing Windows folders. Photos, processing and summaries stay local. Website integration is deferred.

## Current status

The existing local dashboard now connects to a native Rust job worker in its Processing tab. Select a shoot folder, choose its content type, and start. The worker supports JPEG and Canon CR3 inputs, persists progress in a separate SQLite database, provides cancellation and resume, and produces checksum-verified output copies without changing originals. Results and a downloadable JSON report appear in the dashboard; the completion handler submits a Windows notification linking back to Processing.

The Rust worker now calls the existing OpenCV and RapidOCR helpers, reads EXIF from decoded inputs, and reuses exact-byte archive identity metadata through a read-only connection. OCR output stays unconfirmed; conflicting identity labels remain Unknown. Historical grades are never read for a new grade or overwritten.

Rubric 14 category arithmetic, dependencies, motion rules, technical ceilings and the whole-aircraft 79 cap are implemented in Rust. Final rubric grades control naming and routing. **HyperIQA is a separate diagnostic and no longer determines output names or folders.**

**Automatic visual review is implemented using local Qwen3-VL-2B.** Start a shoot in Processing; the model inspects the overview and native detail crops, proposes six rubric categories and observations, and Rust validates the assessment before naming/routing verified copies. Automatic subject detection is the default. Manual scoring is not required in the normal path. Model failures or insufficient evidence remain Unrated rather than receiving invented grades. See [automatic reviewer setup and limits](docs/AUTOMATIC-REVIEWER.md).

Fifteen Rust component tests and ten reviewer component tests pass. The optimized local vision call measured about 8x faster on a generated-image test; see [performance results and limits](docs/REVIEWER-PERFORMANCE.md). This is not a full-photo throughput estimate. Score comparisons and full-shoot end-to-end acceptance remain deferred at Robert's request. The model's agreement with past grades, actual notification delivery and full batch behavior remain unverified. No installer is provided.

See [local integration](docs/LOCAL-INTEGRATION.md), the [implementation plan](docs/PLAN.md), and the earlier [scoring proof](docs/SCORER-PROOF.md). Private dashboard data, downloaded runtimes and model weights are not included.

## Scope

Local dashboard, Windows processing, JPEG and Canon CR3. Website integration, cloud services, remote controls, accounts, native mobile apps, hosted AI, publishing integrations and a Lightroom replacement are excluded from the current phase.

## Repository contents

Only source, documentation, synthetic test fixtures and build metadata belong here. Real photos, personal collection data, local configuration, credentials, downloaded models, runtimes and generated outputs stay outside Git.

## License

A project license has not been selected. This public repository does not yet grant an open-source license. Licenses for any future reused components and model weights must be retained and reviewed separately.
