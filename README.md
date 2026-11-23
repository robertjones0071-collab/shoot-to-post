# Shoot to Post

A local aviation photography dashboard, with a Windows Rust workflow for one-button shoot review.

**Workflow: Open the local dashboard → Select a shoot → Cull Through Review → Receive a local notification and summary.**

Version 1 is planned to support JPEG and Canon CR3 RAW inputs, technical evaluation using the existing rubric, evidence-backed identification, safe naming, verified output copies and a shoot summary. Original input files remain unchanged in the existing Windows folders. Photos, processing and summaries stay local. Website integration is deferred.

## Current status

The existing local dashboard now connects to a native Rust job worker in its Processing tab. Select a shoot folder, choose its content type, and start. The worker supports JPEG and Canon CR3 inputs, persists progress in a separate SQLite database, provides cancellation and resume, and produces checksum-verified output copies without changing originals. Results and a downloadable JSON report appear in the dashboard; the completion handler submits a Windows notification linking back to Processing.

HyperIQA supplies **provisional raw model scores**, not calibrated aviation rubric grades. Existing filename labels are reused; missing airline, aircraft or registration values remain Unknown. This version does not provide automatic aircraft identification or the complete technical rubric. Existing archive ratings and human decisions remain separate.

The Windows release builds and six component tests pass. A real CR3 decoder-only check produced a full-resolution TIFF without changing the original. Dashboard/API readiness is checked separately. **Score comparisons and full-shoot end-to-end acceptance are deferred at Robert's request.** Actual notification delivery and the complete batch lifecycle therefore remain unverified. No installer is provided.

See [local integration](docs/LOCAL-INTEGRATION.md), the [implementation plan](docs/PLAN.md), and the earlier [scoring proof](docs/SCORER-PROOF.md). Private dashboard data, downloaded runtimes and model weights are not included.

## Scope

Local dashboard, Windows processing, JPEG and Canon CR3. Website integration, cloud services, remote controls, accounts, native mobile apps, hosted AI, publishing integrations and a Lightroom replacement are excluded from the current phase.

## Repository contents

Only source, documentation, synthetic test fixtures and build metadata belong here. Real photos, personal collection data, local configuration, credentials, downloaded models, runtimes and generated outputs stay outside Git.

## License

A project license has not been selected. This public repository does not yet grant an open-source license. Licenses for any future reused components and model weights must be retained and reviewed separately.
