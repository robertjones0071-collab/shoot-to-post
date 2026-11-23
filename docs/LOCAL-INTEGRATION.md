# Local dashboard integration

The Rust worker is `src/bin/shoot-to-post-worker.rs`; reusable inference and CR3 rendering are in `src/lib.rs`. `dashboard/shoot-api.mjs` is the Node adapter and `dashboard/shoot-ui.js` adds controls to the existing Processing section (`review-page`). Its stylesheet is `dashboard/shoot-ui.css`.

The private dashboard imports `handleShootRequest` in its HTTP server before its other routes. Refresh embeds the UI script and stylesheet into its snapshots. Static file snapshots link to the live local dashboard for processing. This repository does not contain the private dashboard or archive.

## Runtime requirements

Build using `tools/build-scorer.ps1` (or `-Gnu` on the configured Windows GNU toolchain). The current GNU setup additionally uses a MinGW assembler under `.local/binutils/mingw64/bin` and a SQLite import library `.local/sqlite/libsqlite3.a` with matching `sqlite3.dll`. These local build dependencies are not bundled; a fresh checkout needs them provisioned. Standard Windows builds need SQLite development libraries available to libsqlite3-sys.

Runtime assets are private local files: `.local/hyperiqa.onnx`, `.local/hyperiqa.source.json`, `.local/export-packages/onnxruntime/capi/onnxruntime.dll`, `.local/sqlite/sqlite3.dll`, and the official LibRaw 0.22.3 Windows distribution at `.local/libraw/LibRaw-0.22.3` (including its DLLs and license notices). See SCORER-PROOF.md for model export. Model weights are not redistributed.

The adapter creates `.local/worker.json` with a random worker secret, approved local roots, model checksum and current output folder roles from the existing dashboard configuration. Keep this file private. It launches the release worker hidden on loopback port 47834 when Processing connects. The existing dashboard listens on port 47831. Neither service is exposed to the network.

## Job behavior

Jobs and per-photo results persist in `.local/jobs.sqlite`, separate from the archive. Only one shoot runs at a time. Repeating a Start request with the same request ID returns the existing job. Interrupted runs become paused; resume skips completed items. Cancel takes effect after the current photo. Failed items can be retried.

Choose a shoot directory beneath an approved root, rather than the entire output drive. The output root must be outside the source shoot directory. Junctions and symlinks are rejected. Outputs use safe suffixes and verified copies; existing unrelated files are never overwritten. CR3 files are rendered to temporary TIFFs for scoring, while routed CR3 outputs preserve original bytes.

Final **rubric 14** grades route aviation photos to Gallery at 92+, Under 80 Review below 80, Updated Photos at 80-91 with a known registration, or Unknown Registration Review at 80-91 with an unknown registration. The user's Non-aviation selection routes graded non-aircraft photos to Non-aviation Review. This declared content type is not an automatic classifier. No reliable rubric assessment means Unrated, no scored name, and no output copy. HyperIQA never controls routing or names.

## Existing evidence tools

The adapter supplies a private `tools` configuration containing the existing analysis/OCR Python executables, technical and registration helper scripts, archive database, and this repository's `analysis/collect_evidence.py` bridge. No image-analysis code is rewritten merely to use Rust.

The bridge collects native full-frame measurement tiles (background included, not detected aircraft zones), EXIF from decoded files, OCR suggestions from a bounded full-frame preview, and exact-SHA archive identity metadata. Saved user registration confirmations take precedence over filename-derived archive metadata. Conflicts stay Unknown. No historical score is selected. RAW evidence refers to the temporary full-resolution render and is bound to the original CR3 checksum; LibRaw rendering may omit original camera EXIF, which remains unknown in that case. OCR preview downsizing can miss small text. Suggestions are never auto-confirmed.

## Visual assessment and reports

`src/review.rs` enforces rubric 14 arithmetic and dependencies, one motion mode, PAN anchor evidence, FPAN severity ceilings, failed-zone gates, processing/exposure/color/noise ceilings, 93/94/96/elite requirements, and the separate boundary-truncation cap. Confidence widens uncertainty; it does not impose an automatic quality deduction. The evidence-ledger owner and category observations remain in each report.

The required observations are **not inferred from OpenCV metrics or HyperIQA**; the local Qwen3-VL reviewer now supplies them automatically. See [automatic reviewer](AUTOMATIC-REVIEWER.md). When they are absent the photo becomes `needs_review`; a shoot with such files becomes `awaiting_review`. Automatic Qwen assessments use an overview plus model-grounded native detail crops, with typed Rust validation and one correction retry. The report explains missing evidence and helper errors rather than inventing scores.

A named reviewer can provide a JSON submission using `examples/review-submission.json` as a format reference. Its placeholder text must be replaced with actual observations; the example is not a real assessment. Use the photo ordinal and original SHA-256 from the downloaded report, rubric version 14, six category scores and evidence, five zones, motion/completeness, confidence/range, defect ledger and diagnosis. Optional identification overrides need labels plus supporting evidence. Load it through **Load completed visual review** or authenticated POST `/jobs/{id}/review`. Completed/planned copies cannot be silently rescored. Reviews for changed bytes are rejected. Importing never copies a photo; explicit Resume applies accepted reviews.

The expandable per-photo report displays category scores, raw/technical/final grades, gates, confidence range, zones, primary flaw, OCR suggestions and unresolved identity. Download the full JSON report or CSV manifest. The manifest includes one row per supported/unsupported input, unrated and failed states, source/destination names and sizes, checksums, and grading context. Unknown metadata stays explicit. Native tool provenance and complete observations remain in JSON. CSV export protects formula-like values.

## Verification boundary

Fifteen Rust component tests cover existing preprocessing/copy/recovery behavior plus rubric dependencies and ceilings, failed pans/zones, framing, uncertainty, duplicate-ledger rejection and unrated/no-output behavior. `tests/check_existing_tools.py <dashboard-directory>` checks the real helper bridge and read-only identity conflict handling on generated fixtures, with no shoot job or real-photo grade.

No actual-photo score comparisons, full-shoot acceptance or actual notification delivery checks were performed in this update. These remain deferred. Passing component checks does not establish accuracy of future visual assessments or full-batch readiness.
