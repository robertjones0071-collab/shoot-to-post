# Shoot to Post — Windows application plan

Status: proposed for Robert's review. Windows, JPEG plus Canon CR3 input, and the public `shoot-to-post` GitHub repository are confirmed. Application implementation has not started.

## The product

Select all photos from one shoot, click **Cull Through Review**, and receive a Windows notification when the results are ready. Opening the result shows the shoot summary and the reviewed photos. The application performs the existing review workflow; it does not introduce a different photography workflow.

Windows is the agreed first platform. GitHub is now part of the project, replacing the earlier local-only deferral. Robert confirmed repository name `shoot-to-post` and public visibility. Working application name: **Shoot to Post**.

## Exactly what version 1 does

1. **Select a shoot.** Choose a folder containing JPEGs, Canon CR3 RAW files, or both. Show the number of supported images, unreadable files, and unsupported files before starting. Both JPEG and CR3 are required in version 1. Decode CR3 into a full-resolution review rendering with recorded decoder/settings; an embedded JPEG preview alone is not proof of native-detail RAW review. Do not modify a RAW file or mislabel its rendered appearance as an assessment of unprocessed sensor data. Preserve separate provenance for RAW/JPEG inputs; any proposed pairing must not silently skip a supplied file.
2. **Start one job.** One button starts import, review, identification, scoring, naming, output organization, and summary generation. Initial setup selects the output location; it does not recur as a per-photo approval step.
3. **Review every input.** Keep input ordering deterministic. Compare similar frames to help identify the strongest candidates, but score each independently. Similarity must not invent score separation or automatically delete duplicates. The dashboard's existing above-80 indexing filter must not skip photos in a newly submitted shoot.
4. **Apply the existing evaluation rules.** Examine the full frame and native-detail regions; classify motion, aircraft completeness, visible zones, technical defects, category scores, gates, and uncertainty. Return Unrated when evidence is insufficient. Keep the rubric version, model version, source checksum, evidence, and assessment together.
5. **Identify and name.** Determine airline, aircraft type, and registration where supported. Use `Unknown` rather than inventing missing details. Preserve raw OCR suggestions separately from accepted identification. Use the agreed `Airline_Type_Registration_Score` naming convention, with safe duplicate suffixes.
6. **Organize verified output copies.** Use the confirmed routing below. Preserve original input files, their names, and their bytes. Stage and verify each output before publishing it; never silently overwrite a file. Keep a durable operation record.
7. **Notify and summarize.** Send a Windows notification only after finalizing the job report. Show total inputs, verified output copies, destination counts, unresolved identification, Unrated files, and failures. The notification opens the shoot results when supported by the installed application.

## Confirmed rules to preserve

The routing table below reflects the confirmed Shoot to Post workflow. Older scripts and generic defaults do not override it.

| Result | Destination |
| --- | --- |
| Aviation, final score 92–100, known or unknown registration | Folder 2 — Gallery |
| Aviation, final score 80–91, known registration | Folder 3 — Updated Photos |
| Aviation, final score 80–91, unknown registration | Folder 4 — Unknown Registration Review |
| Non-aviation with a valid assessment | Folder 5 — Non-aviation Review; retain score |
| Aviation, final score 0–79, known or unknown registration | Folder 6 — Under 80 Review |
| Unrated, unsupported, unreadable, or unresolved subject classification | Report as needing attention; do not invent a score or force a scored destination |

Uncertain registration alone does not make a valid technical score Unrated. It uses the Unknown routing above. Unrated files retain their original names and remain available in the shoot results without adding a new numbered workflow folder.

Technical evaluation uses Robert's existing rubric, currently version 14: Detail 30, Focus and motion 25, Exposure 15, Noise 10, Color 10, Processing 10. Preserve motion modes FRZ/PAN/FPAN/AMB, dependency rules, the lowest applicable technical gate, and the final 79 ceiling for a visibly boundary-truncated primary aircraft. Do not score composition, rarity, difficulty, aesthetic preference, or social-media potential. Intentional background streaking is not a panning defect. Preserve existing archive scores and human choices separately from new job assessments.

Robert still owns backup verification, final Lightroom edits, folder 7 staging, and publishing. Starting the app or opening the archive does not automatically start a review job.

## Interface: three job states, existing archive retained

- **Ready:** select the shoot, see input count and output location, click Cull Through Review.
- **Running:** see the current stage, completed/total files, and any errors; cancel safely. Persist work so interruption does not require starting again.
- **Results:** notification summary, destination counts, photos and reasons, and files needing attention. Existing previews and comparison views provide the detail.

Reuse the accepted dashboard's colors, typography, light/dark modes and archive screens. The existing archive remains usable during development. No redesign of accepted screens is included.

## Architecture and reuse

| Piece | Plan |
| --- | --- |
| Windows desktop interface | Tauri shell, reusing the existing web interface where it fits |
| Application engine | Rust: job orchestration, persistent state, validated result handling, file operations and summary |
| Job storage | SQLite; new shoot jobs and assessments remain distinguishable from existing archive records |
| Existing helpers | Reuse verified EXIF, image decoding, technical measurement, OCR and similarity workers initially; do not rewrite them merely to claim everything is Rust |
| Automated visual reviewer | Evaluate a free local model against real reviewed examples; model and hardware feasibility are unresolved |
| Completion notification | Native Windows notification from the installed application, backed by a durable in-app result |

Rust controls the workflow. The visual model supplies observations; deterministic validation enforces score arithmetic, gates, schema and routing. A model's plausible-looking response is not proof of a correct review.

The existing Codex skill is a rubric and operating procedure, not a bundled model or an autonomous background service. The old PowerShell batch mover requires a supplied review plan and has obsolete routing/move behavior; it must not be executed unchanged inside the new application. Current OpenCV workers explicitly produce measurements rather than scores. Current OCR analyzes selected regions; unattended review must supply and validate those regions.

No hosted AI dependency, paid service, or photo upload is approved. If a free local model cannot meet the agreed review quality and hardware needs, stop at that finding and discuss the tradeoff before changing the product or adding a provider.

## Build order and evidence

### 1. Agree on this plan

Windows, JPEG plus CR3, public visibility and repository name are confirmed. Robert still reviews the complete implementation plan before application coding. Record any scope changes here first.

### 2. Establish review quality before a large application build

Use a small, explicitly selected reference shoot with full-size JPEGs and real CR3 files, including RAW/JPEG pairs, RAW orientation, rendering and decoder-failure cases. Include frozen action, successful/failed pans, cropped aircraft, difficult registrations, and relevant score boundaries. Historical ratings are reference evidence, not automatically an adjudicated ground truth. Obtain Robert's agreement on reference outcomes and acceptable differences before evaluating acceptance.

Compare a candidate local reviewer against those references and repeated blinded runs. Check actual crop/motion/gate behavior, category arithmetic, identification errors, Unrated handling, runtime and peak memory on this PC. Report measured results. Do not claim that a Rust rewrite alone improves model quality or inference speed.

This is a release gate for unattended review, not an optional accuracy report to add after shipping. A failed model evaluation leaves the one-button requirement unfinished.

### 3. Build the first end-to-end job

Implement a persisted Rust job and connect the selected validated reviewer and existing helpers. Run one small shoot through input selection, every-photo review, score validation, verified output copies, final manifest, results and Windows notification. Compare it with the agreed reference process. Use an isolated output folder; leave the live archive intact.

### 4. Prove recovery and package Windows

Test cancellation, app restart, partial output recovery, duplicate input, same-name different-content files, unavailable sources/output disks, invalid model responses and worker failure. Resume must not overwrite unrelated files, double-count results, or report unverified work as complete. Keep a per-photo ledger so failed items can be retried separately. A job with errors reports those errors honestly.

Build and test the actual Windows installer. Verify the installed app's notification, notification activation, local runtime/model availability, and complete review on a realistic shoot. Opening the app must not depend on a Codex runtime path or an active Codex chat. Verify the existing dashboard/history still works.

### 5. Accept the finished workflow

Robert runs a shoot, receives the notification, reviews the photos/summary, and confirms the result meets the agreed process. A working window, mock assessment, simulated notification or successful unit test is not completion of this product.

## GitHub repository

Confirmed repository: `shoot-to-post`, public. Create a clean project directory rather than publishing this entire workspace.

Include application source, the agreed plan and rubric, synthetic fixtures, dependency locks, build instructions, third-party license information and reproducible tests. Add Windows build checks once the application has a working build. Commit the plan first; keep implementation changes reviewable.

Keep photos, real shoot manifests, the current archive database, path-containing inventory/journal files, generated dashboard snapshots containing collection data, caches, downloaded models/runtimes, credentials and local configuration out of Git. The private prototype embeds real collection data, so extract a clean template rather than blindly committing it. Document model downloads with versions/checksums and redistribution constraints. Preserve licenses for reused open-source components. Public distribution and a project license need an explicit choice before release.

Repository creation and a documentation-only first push are authorized by Robert's confirmation of the name and public visibility. Application implementation remains pending review of the full plan. Verify actual remote creation and pushed contents before reporting publication.

## Excluded from this scope

iOS, Android, cloud sync, accounts, payments, Instagram publishing, Lightroom replacement, a new dashboard design, automatic source deletion, unrelated archive rescoring, and a blanket rewrite of every existing helper into Rust.

## Planning references

[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/), [Windows installers](https://v2.tauri.app/distribute/windows-installer/), [notifications](https://v2.tauri.app/plugin/notification/).

This public repository starts with documentation only. Existing private prototype files, photos, assessments and operating environments are not included.
