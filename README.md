# Shoot to Post

A local aviation photography dashboard, with a planned Windows Rust workflow for one-button shoot review.

**Planned workflow: Open the local dashboard → Select a shoot → Cull Through Review → Receive a local notification and summary.**

Version 1 is planned to support JPEG and Canon CR3 RAW inputs, technical evaluation using the existing rubric, evidence-backed identification, safe naming, verified output copies and a shoot summary. Original input files remain unchanged in the existing Windows folders. Photos, processing and summaries stay local. Website integration is deferred.

## Current status

The existing local dashboard remains the interface for now. This repository contains planning documentation, not the private dashboard data or a working standalone review application. The complete implementation plan is awaiting review. No installer or validated autonomous reviewer is available yet.

Read the [implementation plan](docs/PLAN.md) for scope, folder routing, architecture and acceptance gates.

## Intended approach

- Keep the accepted local dashboard and existing Windows folders.
- Add a local Rust job engine after the implementation plan is reviewed.
- Keep dashboard-to-worker communication on the PC.
- Reuse working image-analysis helpers where appropriate.
- Validate a free local visual reviewer before committing to a large application build.
- Preserve scoring evidence, model/rubric versions and original files.
- Resume interrupted jobs and report failures honestly.

## Scope

Local dashboard, Windows processing, JPEG and Canon CR3. Website integration, cloud services, remote controls, accounts, native mobile apps, hosted AI, publishing integrations and a Lightroom replacement are excluded from the current phase.

## Repository contents

Only source, documentation, synthetic test fixtures and build metadata belong here. Real photos, personal collection data, local configuration, credentials, downloaded models, runtimes and generated outputs stay outside Git.

## License

A project license has not been selected. This public repository does not yet grant an open-source license. Licenses for any future reused components and model weights must be retained and reviewed separately.
