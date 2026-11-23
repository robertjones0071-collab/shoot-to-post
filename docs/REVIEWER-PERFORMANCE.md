# Automatic reviewer performance

Measured November 22, 2026, on the existing i5-10500T PC with approximately 16 GB RAM and CPU inference. No real photos were scored, no historical-grade comparisons were made, and no shoot job was started.

## Isolated generated-image results

The benchmark sends a generated red rectangle on white, requires constrained JSON, verifies the answer, disables prompt reuse, and starts a separate model process for each case. Server startup is measured separately. OS file caching and background machine activity were not controlled; each entry is one sample, not a statistical throughput estimate.

| Runtime | Vision encoder | CPU threads | Source image | Inference seconds |
| --- | --- | ---: | ---: | ---: |
| b6907 | F16 | 4 | 256 x 256 | 164.168 |
| b6907 | Q8_0 | 4 | 256 x 256 | 186.148 |
| b6907 | Q8_0 | 6 | 256 x 256 | 140.047 |
| b9000 | F16 | 4 | 512 x 512 | 32.611 |
| b9000 | Q8_0 | 4 | 512 x 512 | 20.906 |
| b9000 | Q8_0 | 6 | 512 x 512 | 22.365 |
| b9000 | Q8_0 | 4 | 256 x 256 (same original input) | 20.868 |

The exact original-input check improved by **7.87x**, from 164.168 to 20.868 seconds, excluding startup. The old runtime used 1,050 prompt tokens; the new profile used 282. The semantic result matched, but JSON formatting/token counts differed (13 versus 21 generated tokens). This is a component measurement, **not full-photo or full-shoot throughput and not evidence of equal grading accuracy**.

## Selected settings

- Official llama.cpp **b9000**, Windows CPU archive SHA-256 `8294e287933d3212aa93a32e1ceb800722bede14d9692d405679d4ba77cf05db`.
- Existing official Qwen3-VL-2B Q8_0 language weights retained.
- Official **Q8_0 vision encoder**, SHA-256 `f9a68fabba69c3b81e153367b2c7521030b0fa8bb0de400c9599c8e6725f9c82`.
- **Four threads**, one model slot, dynamic image budget 256-1024 visual tokens, and individual 512-pixel native detail crops. Grounding retains the full-frame overview. The language-model context remains 12,288 tokens.
- Disable the newer runtime's optional RAM prompt-cache pool to keep memory bounded; the model remains warm across photos and is released by the existing idle supervisor.

The previous runtime enlarged even small images to its larger vision input. The new runtime can process native 512-pixel detail crops without that enlargement. Model precision, internal image processing and presentation can change outputs; accuracy/calibration remain deferred.

## Avoided redundant work

Identical native source rectangles are submitted once with all associated zone labels; distinct rectangles and their pixels remain available. The fallback for the old fixed-input runtime packs up to four native crops per contact sheet without resizing. The selected dynamic profile uses individual native crops instead, avoiding a large sheet's attention cost.

A private versioned cache stores only primary-subject locations, never scores. It includes source checksum, language/vision hashes, runtime/thread/image settings, prompt, schema and view profile in its key. A rubric-validation retry can reuse those locations but generates a fresh assessment. Changed inputs/settings invalidate the cache; corrupt cache entries are recomputed. `forceReview` bypasses it at the Python reviewer API.

The model alias now includes encoder, runtime, threads and image limits so an old warm server cannot silently supply a different configuration. Reports record runtime settings and view profile `overview1280-native512-dynamic-v4`.

## Reproduction and limits

`tools/benchmark-vision.py <manifest> --cases f16-4,q8-4,q8-6 --size 512 --min-image-tokens 256 --max-image-tokens 1024` runs the isolated test on port 47836, separate from production. It accepts only the pinned official encoder hashes and saves private timing logs/results under `.local/vision/benchmarks`. The public document contains timings only.

Ten fast reviewer component checks pass. Actual model inference on the generated image and compilation of both full JSON output schemas also pass through the production runtime. Source/rubric/copy safety checks remain separate. The real-photo score comparison, grading calibration and full-shoot acceptance test are still deferred. No under-one-minute full-photo promise is established.
