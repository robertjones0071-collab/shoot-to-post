# Read-only data audit and numerical feasibility experiment

These are research scripts, not app routes or a production scorer. They do not start shoot jobs, rename photos, change archive grades, or rebuild the app. They use existing installed Python packages and model assets.

Run `audit_data.py <workspace> <private-output-directory>`, then `small_pilot.py <workspace> <same-private-output-directory>`. Put output beneath ignored `.local/`. Reports, sample paths, labels, feature rows and predictions are private and must not be committed.

The audit distinguishes journal events from unique originals and image hashes, excludes conflicting hash labels from the pilot pool, examines detailed category coverage, and reads the archive without writes. Snapshots and batch plans are not extra independent examples. Current links use cached inventory hashes; sampled bytes are rechecked before/after processing.

The fixed-seed pilot samples up to nine structured JPEGs per day from four days, one per five-minute capture block, with exact/perceptual duplicate exclusions. The newest day is held out. Imputation/scaling use training rows only; ridge regularization is fixed before measuring held-out results. It uses installed HyperIQA/ONNX Runtime, existing OpenCV measurements and NumPy. The recorded Pillow preprocessing profile is not claimed bit-identical to the Rust triangle-resize profile.

Outputs test numerical speed and reproduction of unadjudicated historical labels. They do not establish photography-quality accuracy, completeness/motion/gate reliability or full-shoot behavior. Do not tune to the tiny held-out set or deploy this pilot as the app scorer. The private report records the measured outcome and limits.
