# Shoot to Post

A planned website dashboard with a local Windows Rust worker for one-button aviation photo review.

**Open the website Dashboard → Select a shoot on the connected PC → Cull Through Review → Receive a notification and summary.**

Version 1 is planned to support JPEG and Canon CR3 RAW inputs, technical evaluation using the existing rubric, evidence-backed identification, safe naming, verified output copies and a shoot summary. Original input files remain unchanged in the existing Windows folders. Photo processing runs locally; the website provides authenticated controls and job summaries.

## Current status

Planning. This repository currently contains documentation, not a working Windows application. The complete implementation plan is awaiting review. No installer or validated autonomous reviewer is available yet.

Read the [implementation plan](docs/PLAN.md) for scope, folder routing, architecture and acceptance gates.

## Intended approach

- Dashboard tab on the existing photography website.
- Installed Rust worker on Windows, paired to the owner and restricted to approved folders.
- Authenticated outbound communication for job requests and status; no publicly exposed local drive.
- Reuse working image-analysis helpers where appropriate.
- Validate a free local visual reviewer before committing to a large application build.
- Preserve scoring evidence, model/rubric versions and original files.
- Resume interrupted jobs and report failures honestly.

## Scope

Website interface, local Windows processing, JPEG and Canon CR3. Owner-only access and minimal job/status synchronization are included. No native mobile app, photo-library cloud sync, hosted AI, publishing integration or Lightroom replacement is included in version 1. Remote photo previews require a separate connection decision; summaries do not require uploading photos.

## Repository contents

Only source, documentation, synthetic test fixtures and build metadata belong here. Real photos, personal collection data, local configuration, credentials, downloaded models, runtimes and generated outputs stay outside Git.

## License

A project license has not been selected. This public repository does not yet grant an open-source license. Licenses for any future reused components and model weights must be retained and reviewed separately.
