//! Calls the existing Python helpers without writing to the archive.
use crate::{file_hash, quiet_command, render_raw};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tools {
    pub analysis_python: PathBuf,
    pub ocr_python: PathBuf,
    pub technical_script: PathBuf,
    pub registration_script: PathBuf,
    pub collector: PathBuf,
    pub archive_database: PathBuf,
    #[serde(default)]
    pub visual_reviewer: Option<PathBuf>,
    #[serde(default)]
    pub vision_manifest: Option<PathBuf>,
}
fn helper(python: &Path, script: &Path, request: Value) -> Result<Value> {
    helper_deadline(python, script, request, Duration::from_secs(180))
}
fn helper_deadline(
    python: &Path,
    script: &Path,
    request: Value,
    timeout: Duration,
) -> Result<Value> {
    let out = tempfile::NamedTempFile::new()?;
    let err = tempfile::NamedTempFile::new()?;
    let mut child = quiet_command(python)
        .arg(script)
        .env("OMP_NUM_THREADS", "1")
        .env("OPENBLAS_NUM_THREADS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(out.reopen()?))
        .stderr(Stdio::from(err.reopen()?))
        .spawn()?;
    let payload = serde_json::to_vec(&request)?;
    let write_result = child
        .stdin
        .take()
        .context("Helper input unavailable")?
        .write_all(&payload);
    if let Err(error) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error.into());
    }
    let started = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Evidence helper timed out");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    if out.as_file().metadata()?.len() > 8 * 1024 * 1024 {
        bail!("Evidence helper response too large");
    }
    let mut bytes = Vec::new();
    out.reopen()?.read_to_end(&mut bytes)?;
    let result: Value = serde_json::from_slice(&bytes).context("Invalid evidence helper JSON")?;
    if !status.success() || result.get("error").is_some() {
        bail!(
            "Evidence helper: {}",
            result["error"].as_str().unwrap_or("process failed")
        );
    }
    Ok(result)
}
pub fn collect(tools: &Tools, source: &Path, decoder: &Path) -> Result<Value> {
    let sha = file_hash(source)?;
    let is_raw = source
        .extension()
        .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("cr3"));
    let rendering = if is_raw {
        Some(render_raw(source, decoder)?)
    } else {
        None
    };
    let input = rendering.as_ref().map_or(source, |f| f.path());
    let input_sha = file_hash(input)?;
    let mut errors = Vec::new();
    let mut run = |mode: &str, python: &Path, worker: &Path| -> Value {
        match helper(
            python,
            &tools.collector,
            json!({"mode":mode,"source":input,"helper":worker}),
        ) {
            Ok(value) if value["inputSha256"].as_str() == Some(input_sha.as_str()) => value,
            Ok(_) => {
                errors.push(format!("{mode}: input checksum mismatch"));
                Value::Null
            }
            Err(e) => {
                errors.push(format!("{mode}: {e:#}"));
                Value::Null
            }
        }
    };
    let technical = run("technical", &tools.analysis_python, &tools.technical_script);
    let ocr = run("ocr", &tools.ocr_python, &tools.registration_script);
    let identity = match helper(
        &tools.analysis_python,
        &tools.collector,
        json!({"mode":"identity","database":tools.archive_database,"sourceSha256":sha}),
    ) {
        Ok(v) => v,
        Err(e) => {
            errors.push(format!("archive identity: {e:#}"));
            Value::Null
        }
    };
    if file_hash(source)? != sha {
        bail!("Source changed during evidence collection");
    }
    Ok(
        json!({"engine":"existing-local-tools-bridge-v1","sourceSha256":sha,"renderedInputSha256":input_sha,"rawRendered":is_raw,"technical":technical,"registrationOcr":ocr,"archiveIdentity":identity,"errors":errors,
 "missingEvidence":["Reviewed native aircraft zones and completeness","Evidence-backed motion classification","Six category scores with observations and defect severity"],"automaticVisualReviewer":false}),
    )
}

/// Automatic model review is bound to the original bytes, including rendered CR3 inputs.
pub fn automatic_review(
    tools: &Tools,
    source: &Path,
    decoder: &Path,
    evidence: &Value,
    correction: Option<&str>,
) -> Result<Value> {
    let sha = file_hash(source)?;
    let script = tools
        .visual_reviewer
        .as_ref()
        .context("Automatic visual reviewer is not configured")?;
    let manifest = tools
        .vision_manifest
        .as_ref()
        .context("Vision model manifest missing")?;
    let is_raw = source
        .extension()
        .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("cr3"));
    let rendering = if is_raw {
        Some(render_raw(source, decoder)?)
    } else {
        None
    };
    let input = rendering.as_ref().map_or(source, |f| f.path());
    let input_sha = file_hash(input)?;
    let mut result = helper_deadline(
        &tools.analysis_python,
        script,
        json!({"source":input,"manifest":manifest,"ocrLines":evidence["registrationOcr"]["lines"],"correction":correction}),
        Duration::from_secs(1800),
    )?;
    if result["inputSha256"].as_str() != Some(input_sha.as_str()) || file_hash(source)? != sha {
        bail!("Photo changed during automatic visual review");
    }
    result["sourceSha256"] = json!(sha);
    result["rawRendered"] = json!(is_raw);
    Ok(result)
}
pub fn automatic_configured(tools: &Tools) -> bool {
    tools.visual_reviewer.as_ref().is_some_and(|p| p.is_file())
        && tools.vision_manifest.as_ref().is_some_and(|p| p.is_file())
}
