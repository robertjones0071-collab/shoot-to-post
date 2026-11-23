use anyhow::{bail, Context, Result};
use image::{imageops::FilterType, DynamicImage, ImageDecoder, ImageReader};
use ort::{session::Session, value::Tensor};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Write, path::PathBuf, time::Instant};

const CROPS: [(usize, usize); 10] = [
    (0, 0),
    (80, 0),
    (160, 0),
    (0, 144),
    (80, 144),
    (160, 144),
    (0, 288),
    (80, 288),
    (160, 288),
    (40, 72),
];
const PROFILE: &str = "hyperiqa-grid10-triangle-v1";
const PATCH_FLOATS: usize = 3 * 224 * 224;
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn checked_mean(scores: &[f32]) -> Result<f64> {
    if scores.len() != CROPS.len() || scores.iter().any(|s| !s.is_finite()) {
        bail!("Expected ten finite patch scores; refusing incomplete or invalid inference");
    }
    Ok(scores.iter().map(|&s| f64::from(s)).sum::<f64>() / scores.len() as f64)
}

fn prepare(bytes: &[u8]) -> Result<([u32; 2], Vec<f32>)> {
    let reader = ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Jpeg | image::ImageFormat::Png)
    ) {
        bail!("This proof accepts JPEG/PNG; CR3 decoding is not implemented yet");
    }
    let mut decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > 100_000_000 {
        bail!("Image exceeds 100 megapixels");
    }
    let orientation = decoder.orientation()?;
    let mut decoded = DynamicImage::from_decoder(decoder)?;
    decoded.apply_orientation(orientation);
    let dimensions = [decoded.width(), decoded.height()];
    let resized = image::imageops::resize(&decoded.to_rgb8(), 384, 512, FilterType::Triangle);
    let mean = [0.485_f32, 0.456, 0.406];
    let std = [0.229_f32, 0.224, 0.225];
    let mut patches = Vec::with_capacity(CROPS.len() * PATCH_FLOATS);
    for &(left, top) in &CROPS {
        for channel in 0..3 {
            for y in 0..224 {
                for x in 0..224 {
                    let v = resized.get_pixel((left + x) as u32, (top + y) as u32)[channel];
                    patches.push((f32::from(v) / 255.0 - mean[channel]) / std[channel]);
                }
            }
        }
    }
    Ok((dimensions, patches))
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut options = BTreeMap::new();
    while let Some(key) = args.next() {
        if ![
            "--model",
            "--model-sha256",
            "--runtime",
            "--image",
            "--dump-input",
        ]
        .contains(&key.as_str())
        {
            bail!("Usage: shoot-to-post-scorer --model FILE --model-sha256 SHA256 --runtime DLL --image PHOTO [--dump-input NEW_FILE]");
        }
        let value = args.next().context("Missing option value")?;
        if options.insert(key, value).is_some() {
            bail!("Duplicate option");
        }
    }
    let required = |key: &str| options.get(key).with_context(|| format!("Missing {key}"));
    let model_path = PathBuf::from(required("--model")?);
    let source_path = PathBuf::from(required("--image")?);
    let expected = required("--model-sha256")?;
    if expected.len() != 64 || !expected.bytes().all(|c| c.is_ascii_hexdigit()) {
        bail!("Invalid model SHA-256");
    }
    let started = Instant::now();
    let model_bytes = fs::read(&model_path).context("Cannot read ONNX model")?;
    let model_sha = digest(&model_bytes);
    if model_sha != expected.to_ascii_lowercase() {
        bail!("Model checksum mismatch");
    }
    // Load verified model bytes rather than reopening a potentially changed file.
    ort::init_from(required("--runtime")?)
        .with_name("shoot-to-post-scorer")
        .commit()?;
    let mut session = Session::builder()?
        .with_intra_threads(2)?
        .with_inter_threads(1)?
        .commit_from_memory(&model_bytes)?;
    if fs::metadata(&source_path)?.len() > 256 * 1024 * 1024 {
        bail!("Input exceeds 256 MiB");
    }
    let source_bytes = fs::read(&source_path).context("Cannot read source image")?;
    let source_sha = digest(&source_bytes);
    let (dimensions, patches) = prepare(&source_bytes)?;
    if let Some(path) = options.get("--dump-input") {
        // Explicit diagnostic output only. Never overwrite an existing file.
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        for v in &patches {
            file.write_all(&v.to_le_bytes())?;
        }
        file.sync_all()?;
    }
    let mut scores = Vec::with_capacity(CROPS.len());
    for patch in patches.chunks_exact(PATCH_FLOATS) {
        let input = Tensor::from_array(([1_usize, 3, 224, 224], patch.to_vec()))?;
        let output = session.run(ort::inputs![input])?;
        let (_, values) = output[0].try_extract_tensor::<f32>()?;
        if values.len() != 1 {
            bail!("Model returned an unexpected output shape");
        }
        scores.push(values[0]);
    }
    let score = checked_mean(&scores)?;
    if digest(&fs::read(&source_path)?) != source_sha {
        bail!("Source changed during scoring; result discarded");
    }
    println!(
        "{}",
        json!({"model":"hyperiqa-koniq", "modelSha256":model_sha, "sourceSha256":source_sha,
        "preprocessing":PROFILE, "orientedDimensions":dimensions, "cropCoordinates":CROPS,
        "patchScores":scores, "rawQualityScore":score, "rubricScore":null,
        "elapsedMilliseconds":started.elapsed().as_millis(), "sourceUnchanged":true,
        "limits":["General image quality; not a calibrated aviation rubric score", "Resized fixed patches; not native-detail aircraft-zone inspection", "No archive scores or files changed"]})
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_results_cannot_become_a_score() {
        assert!(checked_mean(&[1.0; 9]).is_err());
        let mut values = [1.0; 10];
        values[4] = f32::NAN;
        assert!(checked_mean(&values).is_err());
        assert_eq!(checked_mean(&[-2.0; 10]).unwrap(), -2.0);
    }
    #[test]
    fn preprocessing_is_reproducible() {
        let image = image::RgbImage::from_pixel(20, 10, image::Rgb([255, 0, 127]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let (size, tensor) = prepare(bytes.get_ref()).unwrap();
        assert_eq!(size, [20, 10]);
        assert_eq!(tensor.len(), 10 * PATCH_FLOATS);
        assert!((tensor[0] - (1.0 - 0.485) / 0.229).abs() < 1e-6);
        assert!((tensor[224 * 224] - (0.0 - 0.456) / 0.224).abs() < 1e-6);
        assert_eq!(
            &tensor[..PATCH_FLOATS],
            &tensor[PATCH_FLOATS..2 * PATCH_FLOATS]
        );
        assert!(prepare(b"invalid image").is_err());
    }
}
