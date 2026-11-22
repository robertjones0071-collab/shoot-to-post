# Shoot to Post

A planned Windows application for one-button aviation photo review.

**Select a shoot → Cull Through Review → Receive a notification → Review the results.**

Version 1 is planned to support JPEG and Canon CR3 RAW inputs, technical evaluation using the existing rubric, evidence-backed identification, safe naming, verified output copies and a shoot summary. Original input files remain unchanged.

## Current status

Planning. This repository currently contains documentation, not a working Windows application. The complete implementation plan is awaiting review. No installer or validated autonomous reviewer is available yet.

Read the [implementation plan](docs/PLAN.md) for scope, folder routing, architecture and acceptance gates.

## Intended approach

- Rust job engine and a Tauri Windows interface.
- Reuse working image-analysis helpers where appropriate.
- Validate a free local visual reviewer before committing to a large application build.
- Preserve scoring evidence, model/rubric versions and original files.
- Resume interrupted jobs and report failures honestly.

## Scope

Windows, JPEG and Canon CR3. No mobile app, cloud service, publishing integration or Lightroom replacement is included in version 1.

## Repository contents

Only source, documentation, synthetic test fixtures and build metadata belong here. Real photos, personal collection data, local configuration, credentials, downloaded models, runtimes and generated outputs stay outside Git.

## License

A project license has not been selected. This public repository does not yet grant an open-source license. Licenses for any future reused components and model weights must be retained and reviewed separately.
