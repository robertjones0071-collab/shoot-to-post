pub mod compact;
pub mod evidence;
pub mod review;
use anyhow::{bail, Context, Result};
use image::{imageops::FilterType, DynamicImage, ImageDecoder, ImageReader};
use ort::{session::Session, value::Tensor};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

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
pub fn digest(bytes: &[u8]) -> String {
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
        Some(image::ImageFormat::Jpeg | image::ImageFormat::Png | image::ImageFormat::Tiff)
    ) {
        bail!("Unsupported rendered image format");
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

pub fn file_hash(path: &Path) -> Result<String> {
    let mut input = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 1024 * 128];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub fn quiet_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

pub fn render_raw(source: &Path, decoder: &Path) -> Result<tempfile::NamedTempFile> {
    let rendered = tempfile::NamedTempFile::new()?;
    let errors = tempfile::NamedTempFile::new()?;
    let mut child = quiet_command(decoder)
        .args(["-w", "-W", "-q", "3", "-o", "1", "-6", "-T", "-Z", "-"])
        .arg(source)
        .stdout(Stdio::from(rendered.reopen()?))
        .stderr(Stdio::from(errors.reopen()?))
        .spawn()?;
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() > Duration::from_secs(180) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("RAW rendering timed out");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    if !status.success() {
        let mut message = String::new();
        errors.reopen()?.take(4096).read_to_string(&mut message)?;
        bail!("RAW decoding failed: {}", message.trim());
    }
    if rendered.as_file().metadata()?.len() == 0 {
        bail!("RAW decoder produced no image");
    }
    Ok(rendered)
}

pub struct Scorer {
    session: Session,
    pub model_sha: String,
}
impl Scorer {
    pub fn new(model: &Path, expected: &str, runtime: &Path) -> Result<Self> {
        if expected.len() != 64 || !expected.bytes().all(|c| c.is_ascii_hexdigit()) {
            bail!("Invalid model SHA-256");
        }
        let bytes = fs::read(model)?;
        let model_sha = digest(&bytes);
        if model_sha != expected.to_ascii_lowercase() {
            bail!("Model checksum mismatch");
        }
        if !runtime.is_file() {
            bail!("ONNX Runtime DLL is unavailable");
        }
        ort::init_from(runtime.to_string_lossy())
            .with_name("shoot-to-post")
            .commit()?;
        let session = Session::builder()?
            .with_intra_threads(2)?
            .with_inter_threads(1)?
            .commit_from_memory(&bytes)?;
        Ok(Self { session, model_sha })
    }
    pub fn score(
        &mut self,
        source: &Path,
        raw_decoder: Option<&Path>,
        dump: Option<&Path>,
    ) -> Result<serde_json::Value> {
        let started = Instant::now();
        let source_sha = file_hash(source)?;
        let is_raw = source
            .extension()
            .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("cr3"));
        let rendering = if is_raw {
            Some(render_raw(
                source,
                raw_decoder.context("Configure the CR3 decoder first")?,
            )?)
        } else {
            None
        };
        let input_path = rendering.as_ref().map_or(source, |f| f.path());
        if fs::metadata(input_path)?.len() > 768 * 1024 * 1024 {
            bail!("Rendered image exceeds 768 MiB");
        }
        let (dimensions, patches) = prepare(&fs::read(input_path)?)?;
        if let Some(path) = dump {
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
            let outputs = self.session.run(ort::inputs![input])?;
            let (_, values) = outputs[0].try_extract_tensor::<f32>()?;
            if values.len() != 1 {
                bail!("Unexpected model output shape");
            }
            scores.push(values[0]);
        }
        let score = checked_mean(&scores)?;
        if file_hash(source)? != source_sha {
            bail!("Source changed during scoring");
        }
        Ok(
            json!({"model":"hyperiqa-koniq","modelSha256":self.model_sha,"sourceSha256":source_sha,
            "preprocessing":PROFILE,"orientedDimensions":dimensions,"cropCoordinates":CROPS,"patchScores":scores,
            "rawQualityScore":score,"rubricScore":null,"sourceUnchanged":true,"elapsedMilliseconds":started.elapsed().as_millis(),
            "rawRendering":if is_raw {json!({"decoder":"LibRaw dcraw_emu","settings":["-w","-W","-q","3","-o","1","-6","-T","-Z","-"],"fullResolution":true})} else {serde_json::Value::Null},
            "limits":["Provisional general image-quality score; not a calibrated aviation rubric score","No native-detail aircraft-zone diagnosis","Historical scores unchanged"]}),
        )
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
    #[test]
    fn sixteen_bit_raw_render_is_supported_without_a_scoring_run() {
        let image = image::ImageBuffer::<image::Rgb<u16>, _>::from_pixel(
            12,
            8,
            image::Rgb([65535, 32768, 0]),
        );
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb16(image)
            .write_to(&mut bytes, image::ImageFormat::Tiff)
            .unwrap();
        let (dimensions, tensor) = prepare(bytes.get_ref()).unwrap();
        assert_eq!(dimensions, [12, 8]);
        assert_eq!(tensor.len(), 10 * PATCH_FLOATS);
    }
}
