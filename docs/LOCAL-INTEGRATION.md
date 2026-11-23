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

Rounded provisional HyperIQA values route aviation photos to Gallery at 92+, Under 80 Review below 80, or Updated Photos at 80–91 when the registration label is known. Missing registration at 80–91 routes to Unknown Registration Review. The user's Non-aviation selection routes to Non-aviation Review. This declared content type is not an automatic classifier. Filename metadata is retained where available; missing labels stay Unknown. Output thresholds have not been calibrated against historical grades.

The results view reports progress, errors, destinations and identification gaps. Download shoot report saves the complete local JSON result. The Windows toast handler records submission or an error; Windows notification settings can affect popup visibility. Its link opens the local Processing tab.

## Verification boundary

Six component tests cover preprocessing, invalid score rejection, 16-bit render decoding, routing boundaries, copy integrity/no overwrite, persisted recovery and idempotent Start. A real CR3 decoder-only check passed. No new actual-photo score comparisons or full-shoot acceptance runs were performed during this integration, as requested. Notification delivery and full batch completion must be checked when that deferred acceptance phase is authorized.
