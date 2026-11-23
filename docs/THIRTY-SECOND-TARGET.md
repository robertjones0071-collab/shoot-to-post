# Thirty-second reviewer gate: failed

The goal is a successful, automatic technical assessment of a typical JPEG, complete with rubric validation and a checksum-verified copy, within 30 seconds. Finishing quickly as Unrated or replaying an old grade is not a pass.

## Implemented candidate

- Reuse checksum-matched, helper-version-matched OpenCV/OCR evidence. Failed assessments never become reusable grades.
- Cache only accepted, currently valid assessments, keyed by original bytes, model manifest, helper/prompt/schema code, compact policy and Rust rubric code. Cache reads repeat typed rubric validation. Outputs are still independently copied/verified for the new job.
- Replace the two-pass prose-producing reviewer with one compact observation call. Rust selects explicit conservative point anchors within existing rubric bands, applies all existing dependencies/ceilings and generates the report descriptions. These anchor choices are a new, **uncalibrated** policy (`rubric14-conservative-band-v1`), not evidence of historical-grade agreement.
- Preserve an original-frame overview and native inspection samples. Compatible previous subject locations may be reused; unknown images get labelled generic samples, not a fabricated aircraft detector. Missing category evidence means Unrated.
- Restrict aircraft completeness by subject class in the response contract, preventing the former not-applicable-for-aircraft response from being accepted.
- Bound output to 192 tokens and a 90-second model request. Where safe, repair only a failed framing/motion field using at most 24 tokens and 25 seconds. Other unsupported evidence/invalid results remain Unrated; never regenerate an entire lengthy assessment to force a pass.
- Record evidence, model startup, view preparation, model observation, total-item and copy timings. Skip optional HyperIQA inference on compact observations because it does not determine the rubric grade.

## Same-photo acceptance test

One isolated run used the exact same 6000 x 3368 JPEG bytes as the prior test. Model startup was cold. Existing measurements and subject locations were reused, but the visual assessment was fresh and no accepted-grade cache was used. Outputs were confined to a separate test directory. No historical-score comparison or full-shoot test was performed.

| Measurement | Seconds |
| --- | ---: |
| End-to-end, including job completion/notification submission | 89.870 |
| Existing evidence reuse | 0.001 |
| Model startup | 17.855 |
| View preparation | 0.578 |
| Model observation request | 70.121 |
| Total item processing | 89.531 |

The model emitted 142 response tokens and marked all six category bands unassessable. Rust left the photo **Unrated**; no scored output was made. There was one model attempt, no accepted-assessment cache hit, and no score comparison. Original bytes and saved archive judgments were verified unchanged.

**Result: speed and usable-assessment gates failed.** The compact candidate is not promoted to the dashboard's default reviewer. The existing production binary/configuration was not replaced by this test. The source contains an experimental compact path; its tests establish contract and safety behavior, not grading accuracy.

The 30-second goal is not met using the installed model on this CPU. Any further model/hardware decision is a separate next step; this result must not be presented as a working 30-second reviewer.

## Verification

19 Rust component tests and compact Python contract checks cover gates, missing evidence, accepted-cache rejection/invalidation, one-call observations, class-specific framing and field-only repair. The compact schema was also compiled by the installed model backend before timing. A successful full real-photo review, historical-score agreement and full-shoot behavior remain unproven.
