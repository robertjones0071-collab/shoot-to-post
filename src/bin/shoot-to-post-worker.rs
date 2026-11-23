use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shoot_to_post_scorer::{
    compact, digest,
    evidence::{self, Tools},
    file_hash,
    review::{self, VisualReview},
    Scorer,
};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tiny_http::{Header, Method, Response, Server, StatusCode};

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Settings {
    bind: String,
    token: String,
    database: PathBuf,
    allowed_roots: Vec<PathBuf>,
    model: PathBuf,
    model_sha256: String,
    runtime: PathBuf,
    raw_decoder: PathBuf,
    default_output: PathBuf,
    folder_names: BTreeMap<String, String>,
    #[serde(default)]
    tools: Option<Tools>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Job {
    id: String,
    source: PathBuf,
    output: PathBuf,
    kind: String,
    state: String,
    created_at: u128,
    total: usize,
    unsupported: usize,
    #[serde(default)]
    unsupported_paths: Vec<PathBuf>,
    error: Option<String>,
    notification: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    ordinal: usize,
    source: PathBuf,
    state: String,
    source_sha: Option<String>,
    score: Option<f64>,
    output: Option<PathBuf>,
    labels: Option<[String; 3]>,
    assessment: Option<Value>,
    error: Option<String>,
    copied: bool,
    #[serde(default)]
    evidence: Option<Value>,
    #[serde(default)]
    visual_review: Option<VisualReview>,
    #[serde(default)]
    identification_review: Option<IdentificationReview>,
    #[serde(default)]
    automatic_review: Option<Value>,
    #[serde(default)]
    effective_kind: Option<String>,
    #[serde(default)]
    stage: Option<String>,
    #[serde(default)]
    timings: BTreeMap<String, f64>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IdentificationReview {
    labels: [String; 3],
    evidence: [String; 3],
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewSubmission {
    ordinal: usize,
    source_sha256: String,
    review: VisualReview,
    identification: Option<IdentificationReview>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NewJob {
    source: PathBuf,
    output: PathBuf,
    kind: String,
    request_id: Option<String>,
}
struct App {
    settings: Settings,
    db: Mutex<Connection>,
    picker_busy: AtomicBool,
}
fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
fn normalized(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}
fn within(path: &Path, root: &Path) -> bool {
    let p = normalized(path);
    let r = normalized(root);
    p == r || p.starts_with(&(r + "\\"))
}
fn approved(path: &Path, settings: &Settings) -> Result<PathBuf> {
    let real = path
        .canonicalize()
        .context("Folder or file is unavailable")?;
    if !settings
        .allowed_roots
        .iter()
        .any(|r| r.canonicalize().is_ok_and(|root| within(&real, &root)))
    {
        bail!("Choose a folder within the configured local roots");
    }
    Ok(real)
}
fn public_path(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .to_string()
}
fn is_link(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}
fn enumerate(
    folder: &Path,
    files: &mut Vec<PathBuf>,
    unsupported: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if is_link(&meta) {
            continue;
        }
        if meta.is_dir() {
            enumerate(&path, files, unsupported)?;
        } else if meta.is_file() {
            let ext = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            if ["jpg", "jpeg", "png", "cr3"].contains(&ext.as_str()) {
                files.push(path);
            } else {
                unsupported.push(path);
            }
        }
        if files.len() > 100_000 {
            bail!("Select a shoot folder rather than the entire archive");
        }
    }
    Ok(())
}
fn load_job(db: &Connection, id: &str) -> Result<Job> {
    let raw: String = db
        .query_row("SELECT data FROM jobs WHERE id=?", [id], |r| r.get(0))
        .context("Job not found")?;
    Ok(serde_json::from_str(&raw)?)
}
fn save_job(db: &Connection, job: &Job) -> Result<()> {
    db.execute("INSERT INTO jobs(id,state,data) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET state=excluded.state,data=excluded.data",params![job.id,job.state,serde_json::to_string(job)?])?;
    Ok(())
}
fn save_item(db: &Connection, id: &str, item: &Item) -> Result<()> {
    db.execute("INSERT INTO items(job_id,ordinal,state,data) VALUES(?,?,?,?) ON CONFLICT(job_id,ordinal) DO UPDATE SET state=excluded.state,data=excluded.data",params![id,item.ordinal,item.state,serde_json::to_string(item)?])?;
    Ok(())
}
fn items(db: &Connection, id: &str) -> Result<Vec<Item>> {
    let mut q = db.prepare("SELECT data FROM items WHERE job_id=? ORDER BY ordinal")?;
    let rows = q.query_map([id], |r| r.get::<_, String>(0))?;
    rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
}
fn summary(db: &Connection, job: &Job, detail: bool) -> Result<Value> {
    let rows = items(db, &job.id)?;
    let done = rows.iter().filter(|r| r.state == "done").count();
    let failed = rows.iter().filter(|r| r.state == "failed").count();
    let needs_review = rows.iter().filter(|r| r.state == "needs_review").count();
    let attention = rows
        .iter()
        .filter(|r| {
            r.labels
                .as_ref()
                .is_some_and(|labels| labels.iter().any(|l| l == "Unknown"))
        })
        .count();
    let mut folders: BTreeMap<String, usize> = BTreeMap::new();
    for r in &rows {
        if r.state == "done" {
            if let Some(path) = &r.output {
                if let Some(folder) = path.parent().and_then(|p| p.file_name()) {
                    *folders
                        .entry(folder.to_string_lossy().to_string())
                        .or_default() += 1;
                }
            }
        }
    }
    let mut value = json!({"job":job,"completed":done,"failed":failed,"processed":done+failed+needs_review,"needsReview":needs_review,"needsIdentification":attention,"destinations":folders,"scoreBasis":"Evidence-backed rubric 14; HyperIQA diagnostic only; calibration deferred"});
    if detail {
        let mut manifest:Vec<Value>=rows.iter().map(|r|{
            let labels=r.labels.clone().unwrap_or(["Unknown".into(),"Unknown".into(),"Unknown".into()]);
            let rubric=r.assessment.as_ref().map(|a|a["rubric"].clone()).unwrap_or(Value::Null);
            let source_bytes=fs::metadata(&r.source).ok().map(|m|m.len());
            let destination_bytes=r.output.as_ref().and_then(|p|fs::metadata(p).ok()).map(|m|m.len());
            json!({"originalFilename":r.source.file_name().map(|s|s.to_string_lossy()),"sourcePath":public_path(&r.source),"sourceSha256":r.source_sha,"sourceSizeBytes":source_bytes,
            "newFilename":r.output.as_ref().and_then(|p|p.file_name()).map(|s|s.to_string_lossy()),"destinationPath":r.output.as_ref().map(|p|public_path(p)),"destinationSizeBytes":destination_bytes,
            "score":r.score,"scoreBand":rubric["scoreBand"],"motionMode":rubric["motionMode"],"confidence":rubric["confidence"],"airline":labels[0],"aircraft":labels[1],"registration":labels[2],"airport":"Unknown",
            "captureDate":r.evidence.as_ref().map(|e|e["technical"]["metadata"]["captureDate"].clone()),"primaryFlaw":rubric["primaryFlaw"],"ratingStatus":if r.score.is_some(){"rated"}else{"unrated"},
            "fileOperationStatus":match r.state.as_str(){"done"=>"renamed_copy_created","failed"=>"failed","needs_review"=>"unrated",_=>"proposed_only"},"error":r.error,"rubricVersion":14})
        }).collect();
        manifest.extend(job.unsupported_paths.iter().map(|p|json!({"sourcePath":public_path(p),"originalFilename":p.file_name().map(|s|s.to_string_lossy()),"score":null,"fileOperationStatus":"unsupported_file","ratingStatus":"unsupported"})));
        value["manifest"] = json!(manifest);
        value["reportVersion"] = json!(2);
        value["items"] = serde_json::to_value(rows)?;
    }
    Ok(value)
}
fn create_job(app: &App, request: NewJob) -> Result<Value> {
    if !["automatic", "aviation", "non-aviation"].contains(&request.kind.as_str()) {
        bail!("Choose automatic detection, aviation or non-aviation for this shoot");
    }
    let source = approved(&request.source, &app.settings)?;
    let output = approved(&request.output, &app.settings)?;
    if !source.is_dir() || !output.is_dir() {
        bail!("Select existing source and output folders");
    }
    if within(&output, &source) {
        bail!("Output must be outside the selected shoot folder");
    }
    let mut paths = Vec::new();
    let mut unsupported = Vec::new();
    enumerate(&source, &mut paths, &mut unsupported)?;
    paths.sort();
    if paths.is_empty() {
        bail!("No JPEG, PNG or CR3 photos found in the selected shoot");
    }
    let db = app.db.lock().unwrap();
    let id = if let Some(key) = request.request_id.as_ref() {
        if key.is_empty()
            || key.len() > 64
            || !key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            bail!("Invalid job request ID");
        }
        format!("shoot-{key}")
    } else {
        format!("shoot-{}", now())
    };
    if let Ok(existing) = load_job(&db, &id) {
        if existing.source != source || existing.output != output || existing.kind != request.kind {
            bail!("A request ID cannot be reused for another shoot");
        }
        return summary(&db, &existing, false);
    }
    if db.query_row(
        "SELECT COUNT(*) FROM jobs WHERE state IN ('queued','running','cancel_requested')",
        [],
        |r| r.get::<_, usize>(0),
    )? > 0
    {
        bail!("Finish or cancel the active shoot first");
    }
    let job = Job {
        id,
        source,
        output,
        kind: request.kind,
        state: "queued".into(),
        created_at: now(),
        total: paths.len(),
        unsupported: unsupported.len(),
        unsupported_paths: unsupported,
        error: None,
        notification: None,
    };
    db.execute_batch("BEGIN IMMEDIATE")?;
    let saved = (|| -> Result<()> {
        save_job(&db, &job)?;
        for (ordinal, path) in paths.into_iter().enumerate() {
            save_item(
                &db,
                &job.id,
                &Item {
                    ordinal,
                    source: path,
                    state: "pending".into(),
                    source_sha: None,
                    score: None,
                    output: None,
                    labels: None,
                    assessment: None,
                    error: None,
                    copied: false,
                    evidence: None,
                    visual_review: None,
                    identification_review: None,
                    automatic_review: None,
                    effective_kind: None,
                    stage: None,
                    timings: BTreeMap::new(),
                },
            )?;
        }
        Ok(())
    })();
    if let Err(error) = saved {
        let _ = db.execute_batch("ROLLBACK");
        return Err(error);
    }
    db.execute_batch("COMMIT")?;
    summary(&db, &job, false)
}
fn route(score: u8, kind: &str, registration: &str) -> u8 {
    if kind == "non-aviation" {
        5
    } else if score >= 92 {
        2
    } else if score < 80 {
        6
    } else if registration == "Unknown" {
        4
    } else {
        3
    }
}
fn slug(value: &str) -> String {
    let result: String = value
        .chars()
        .take(45)
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if result.trim_matches('-').is_empty()
        || result.eq_ignore_ascii_case("unknown")
        || result.eq_ignore_ascii_case("unk")
    {
        "Unknown".into()
    } else {
        result
    }
}
fn labels(path: &Path) -> [String; 3] {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let parts: Vec<_> = stem.split('_').collect();
    if parts.len() >= 4 && parts[3].parse::<u8>().is_ok_and(|s| s <= 100) {
        return [slug(parts[0]), slug(parts[1]), slug(parts[2])];
    }
    ["Unknown".into(), "Unknown".into(), "Unknown".into()]
}
fn verified_copy(source: &Path, destination: &Path, expected: &str) -> Result<bool> {
    if file_hash(source)? != expected {
        bail!("Source changed before output copy");
    }
    if destination.exists() {
        if file_hash(destination)? == expected {
            return Ok(false);
        }
        bail!("Output already exists with different content");
    }
    let parent = destination.parent().context("Missing destination folder")?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    let mut input = fs::File::open(source)?;
    std::io::copy(&mut input, &mut staged)?;
    staged.as_file_mut().sync_all()?;
    if file_hash(staged.path())? != expected || file_hash(source)? != expected {
        bail!("Output checksum mismatch or changed source");
    }
    staged.persist_noclobber(destination).map_err(|e| {
        anyhow::anyhow!("Could not publish output without overwriting: {}", e.error)
    })?;
    if file_hash(destination)? != expected {
        bail!("Published output checksum mismatch");
    }
    Ok(true)
}
fn validated_automatic(automatic: &Value) -> Result<(VisualReview, String)> {
    if automatic["assessable"].as_bool() != Some(true) {
        bail!("Automatic model did not establish an assessable image");
    }
    if automatic.get("compactObservation").is_some() {
        return compact::to_review(automatic);
    }
    let kind = automatic["effectiveKind"]
        .as_str()
        .context("Missing detected subject type")?
        .to_owned();
    if !["aviation", "non-aviation"].contains(&kind.as_str()) {
        bail!("Invalid automatic subject classification");
    }
    let review: VisualReview = serde_json::from_value(automatic["review"].clone())?;
    review::evaluate(&review, kind == "aviation")?;
    Ok((review, kind))
}
fn assessment_cache_key(app: &App, sha: &str) -> Result<(PathBuf, String)> {
    let tools = app.settings.tools.as_ref().context("No tools configured")?;
    let manifest = tools
        .vision_manifest
        .as_ref()
        .context("No model manifest")?;
    let root = manifest.parent().context("Missing model directory")?;
    let mut fingerprint = sha.to_owned();
    for path in [
        manifest,
        tools.visual_reviewer.as_ref().context("No reviewer")?,
        &tools.collector,
        &tools.technical_script,
        &tools.registration_script,
    ] {
        fingerprint.push_str(&file_hash(path)?);
    }
    let analysis = tools
        .visual_reviewer
        .as_ref()
        .unwrap()
        .parent()
        .context("No analysis directory")?;
    for name in [
        "visual_reviewer.py",
        "fast-vision-schema.json",
        "fast-vision-prompt.txt",
    ] {
        fingerprint.push_str(&file_hash(&analysis.join(name))?);
    }
    fingerprint.push_str(include_str!("../compact.rs"));
    fingerprint.push_str(include_str!("../review.rs"));
    let key = digest(fingerprint.as_bytes());
    Ok((
        root.join("accepted-assessments")
            .join(format!("{key}.json")),
        key,
    ))
}
fn accepted_cache(path: &Path, key: &str, sha: &str) -> Option<Value> {
    if fs::metadata(path).ok()?.len() > 8 * 1024 * 1024 {
        return None;
    }
    let value: Value = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    if value["key"].as_str() != Some(key)
        || value["sourceSha256"].as_str() != Some(sha)
        || value["automatic"]["accepted"].as_bool() != Some(true)
        || value["evidence"]["sourceSha256"].as_str() != Some(sha)
    {
        return None;
    }
    validated_automatic(&value["automatic"]).ok()?;
    Some(value)
}
fn save_accepted_cache(path: &Path, value: &Value) -> Result<()> {
    let parent = path.parent().context("Cache parent missing")?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    use std::io::Write;
    file.write_all(&serde_json::to_vec(value)?)?;
    file.as_file_mut().sync_all()?;
    if !path.exists() {
        file.persist_noclobber(path)
            .map_err(|e| anyhow::anyhow!("Cache publish: {}", e.error))?;
    }
    Ok(())
}
fn reusable_evidence(db: &Connection, sha: &str, tools: &Tools) -> Option<Value> {
    let native = file_hash(&tools.technical_script).ok()?;
    let ocr = file_hash(&tools.registration_script).ok()?;
    let mut q = db
        .prepare("SELECT data FROM items WHERE json_extract(data,'$.sourceSha')=? LIMIT 20")
        .ok()?;
    let rows = q.query_map([sha], |r| r.get::<_, String>(0)).ok()?;
    for raw in rows.flatten() {
        let Ok(item) = serde_json::from_str::<Item>(&raw) else {
            continue;
        };
        let Some(mut e) = item.evidence else {
            continue;
        };
        if e["sourceSha256"].as_str() != Some(sha)
            || e["registrationOcr"]["helperSha256"].as_str() != Some(ocr.as_str())
        {
            continue;
        }
        let Some(tiles) = e["technical"]["tiles"].as_array() else {
            continue;
        };
        if tiles.is_empty() {
            continue;
        }
        // The collector stores the helper fingerprint on the technical envelope.
        if e["technical"]["helperSha256"].as_str() != Some(native.as_str()) {
            continue;
        }
        e["reusedSourceHashEvidence"] = json!(true);
        return Some(e);
    }
    None
}
fn process_item(app: &App, job: &Job, item: &mut Item, scorer: Option<&mut Scorer>) -> Result<()> {
    let item_started = std::time::Instant::now();
    let source = approved(&item.source, &app.settings)?;
    // A recorded output plan is a recovery checkpoint, not permission to replace any file.
    if let (Some(destination), Some(expected)) = (&item.output, &item.source_sha) {
        if item
            .assessment
            .as_ref()
            .and_then(|a| a["rubric"]["status"].as_str())
            != Some("rated")
        {
            bail!("Legacy provisional-score output plan requires a new rubric review");
        }
        let root = approved(&job.output, &app.settings)?;
        let parent = approved(
            destination
                .parent()
                .context("Missing saved destination parent")?,
            &app.settings,
        )?;
        if !within(&parent, &root) {
            bail!("Saved output resolves outside the selected root");
        }
        if file_hash(&source)? != *expected {
            bail!("Source changed since this item was planned");
        }
        item.copied = verified_copy(&source, destination, expected)?;
        item.state = "done".into();
        return Ok(());
    }
    let sha = file_hash(&source)?;
    if item.source_sha.as_ref().is_some_and(|old| old != &sha) {
        bail!("Source changed since evidence/review was recorded");
    }
    item.source_sha = Some(sha.clone());
    let cache = assessment_cache_key(app, &sha).ok();
    if item.visual_review.is_none() {
        if let Some((path, key)) = &cache {
            if let Some(saved) = accepted_cache(path, key, &sha) {
                let (mut automatic, evidence) =
                    (saved["automatic"].clone(), saved["evidence"].clone());
                let (review, kind) = validated_automatic(&automatic)?;
                automatic["assessmentCacheHit"] = json!(true);
                automatic["originalElapsedSeconds"] = automatic["elapsedSeconds"].clone();
                automatic["elapsedSeconds"] = json!(0);
                item.visual_review = Some(review);
                item.effective_kind = Some(kind);
                item.evidence = Some(evidence);
                item.automatic_review = Some(automatic);
            }
        }
    }
    let evidence_started = std::time::Instant::now();
    if item.evidence.is_none() {
        if let Some(tools) = &app.settings.tools {
            item.evidence = reusable_evidence(&app.db.lock().unwrap(), &sha, tools);
        }
    }
    if item.evidence.is_none() {
        let tools = app
            .settings
            .tools
            .as_ref()
            .context("Existing analysis tools are not configured")?;
        item.evidence = Some(evidence::collect(
            tools,
            &source,
            &app.settings.raw_decoder,
        )?);
    }
    item.timings.insert(
        "evidenceSeconds".into(),
        evidence_started.elapsed().as_secs_f64(),
    );
    if item.visual_review.is_none() {
        if let Some(tools) = app
            .settings
            .tools
            .as_ref()
            .filter(|t| evidence::automatic_configured(t))
        {
            item.stage = Some("Automatic visual review on local CPU".into());
            save_item(&app.db.lock().unwrap(), &job.id, item)?;
            let mut correction: Option<Value> = None;
            let visual_started = std::time::Instant::now();
            for attempt in 0..2 {
                let mut automatic = evidence::automatic_review(
                    tools,
                    &source,
                    &app.settings.raw_decoder,
                    item.evidence
                        .as_ref()
                        .context("Missing collected evidence")?,
                    correction.as_ref(),
                )?;
                automatic["attempt"] = json!(attempt + 1);
                if let Some(previous) = &item.automatic_review {
                    automatic["previousAttemptError"] = previous["validationError"].clone();
                }
                if automatic["assessable"].as_bool() != Some(true) {
                    item.automatic_review = Some(automatic);
                    break;
                }
                let validated = validated_automatic(&automatic);
                match validated {
                    Ok((review, kind)) => {
                        automatic["accepted"] = json!(true);
                        automatic["review"] = serde_json::to_value(&review)?;
                        item.visual_review = Some(review);
                        item.effective_kind = Some(kind);
                        item.automatic_review = Some(automatic);
                        break;
                    }
                    Err(error) => {
                        let message = format!("{error:#}");
                        automatic["accepted"] = json!(false);
                        automatic["validationError"] = json!(message);
                        item.automatic_review = Some(automatic);
                        let field = if message.starts_with("c:") {
                            Some("c")
                        } else if message.starts_with("m:") || message.contains("Successful PAN") {
                            Some("m")
                        } else {
                            None
                        };
                        if item
                            .automatic_review
                            .as_ref()
                            .is_some_and(|a| a.get("compactObservation").is_some())
                        {
                            let Some(field) = field else {
                                break;
                            };
                            correction = Some(
                                json!({"repairField":field,"previousObservation":item.automatic_review.as_ref().unwrap()["compactObservation"],"correction":message}),
                            );
                        } else {
                            break;
                        }
                    }
                }
            }
            item.timings.insert(
                "visualReviewSeconds".into(),
                visual_started.elapsed().as_secs_f64(),
            );
            save_item(&app.db.lock().unwrap(), &job.id, item)?;
        }
    }
    if item.visual_review.is_some()
        && item
            .automatic_review
            .as_ref()
            .is_some_and(|a| a["accepted"].as_bool() == Some(true))
    {
        if let Some((path, key)) = &cache {
            let _ = save_accepted_cache(
                path,
                &json!({"key":key,"sourceSha256":sha,"automatic":item.automatic_review,"evidence":item.evidence}),
            );
        }
    }
    let effective_kind = item.effective_kind.as_deref().unwrap_or(&job.kind);
    let mut names = labels(&source);
    let mut provenance = vec!["existing filename claim"; 3];
    if let Some(evidence) = &item.evidence {
        for i in 0..3 {
            if evidence["archiveIdentity"]["conflicts"][i].as_bool() == Some(true) {
                names[i] = "Unknown".into();
                provenance[i] = "conflicting exact-byte archive metadata";
            } else if let Some(label) = evidence["archiveIdentity"]["labels"][i]
                .as_str()
                .filter(|s| *s != "Unknown")
            {
                if names[i] != "Unknown" && !names[i].eq_ignore_ascii_case(&slug(label)) {
                    names[i] = "Unknown".into();
                    provenance[i] = "filename/archive conflict";
                } else {
                    names[i] = slug(label);
                    provenance[i] = "exact SHA-256 archive metadata";
                }
            }
        }
    }
    if let Some(automatic) = item
        .automatic_review
        .as_ref()
        .filter(|v| v["accepted"].as_bool() == Some(true))
    {
        let proposed = &automatic["identification"];
        for (i, key) in ["airline", "aircraft", "registration"].iter().enumerate() {
            let Some(label) = proposed[*key]
                .as_str()
                .filter(|s| !s.trim().is_empty() && *s != "Unknown")
            else {
                continue;
            };
            if proposed["evidence"][*key]
                .as_str()
                .is_none_or(|e| e.trim().is_empty())
            {
                continue;
            }
            // Registrations require exact independent OCR agreement as well as visual evidence.
            if i == 2
                && !item
                    .evidence
                    .as_ref()
                    .and_then(|e| e["registrationOcr"]["lines"].as_array())
                    .is_some_and(|lines| {
                        lines.iter().any(|line| {
                            line["registrationLike"].as_bool() == Some(true)
                                && line["normalized"]
                                    .as_str()
                                    .is_some_and(|v| v.eq_ignore_ascii_case(label))
                        })
                    })
            {
                continue;
            }
            let proposed = slug(label);
            if names[i] == "Unknown" && !provenance[i].contains("conflict") {
                names[i] = proposed;
                provenance[i] = "automatic visual identification hypothesis";
            } else if names[i] != "Unknown" && !names[i].eq_ignore_ascii_case(&proposed) {
                names[i] = "Unknown".into();
                provenance[i] = "automatic/existing identity conflict";
            }
        }
    }
    if let Some(identification) = &item.identification_review {
        for i in 0..3 {
            names[i] = slug(&identification.labels[i]);
            provenance[i] = "reviewer-confirmed identification";
        }
    }
    item.labels = Some(names.clone());
    // General quality prediction is retained as a diagnostic, never as a rubric grade.
    let diagnostic = if let Some(previous) = item
        .assessment
        .as_ref()
        .and_then(|a| a.get("modelDiagnostic"))
    {
        previous.clone()
    } else if item
        .automatic_review
        .as_ref()
        .is_some_and(|a| a.get("compactObservation").is_some())
    {
        json!({"status":"skipped","reason":"Optional general-IQA diagnostic does not determine rubric grades"})
    } else if let Some(scorer) = scorer {
        match scorer.score(&source, Some(&app.settings.raw_decoder), None) {
            Ok(v) => v,
            Err(e) => json!({"error":format!("{e:#}")}),
        }
    } else {
        json!({"error":"Optional HyperIQA model unavailable"})
    };
    let result = if let Some(review) = &item.visual_review {
        review::evaluate(review, effective_kind == "aviation")?
    } else {
        json!({"status":"needs_review","rubricVersion":14,"finalScore":null,"reason":item.automatic_review.as_ref().and_then(|v|v["reason"].as_str().or(v["validationError"].as_str())).unwrap_or("Automatic visual reviewer is unavailable or could not support a reliable assessment")})
    };
    item.assessment = Some(
        json!({"sourceSha256":sha,"rubric":result,"automaticModelAssessment":item.automatic_review,"effectiveKind":effective_kind,"modelDiagnostic":diagnostic,"identification":{"labels":names,"basis":provenance,"review":item.identification_review,"registrationOcrAcceptedAutomatically":false}}),
    );
    let Some(final_score) = result["finalScore"].as_u64() else {
        item.score = None;
        item.state = "needs_review".into();
        item.stage = Some("Unrated: automatic evidence insufficient".into());
        item.timings.insert(
            "totalItemSeconds".into(),
            item_started.elapsed().as_secs_f64(),
        );
        return Ok(());
    };
    if file_hash(&source)? != sha {
        bail!("Source changed after assessment");
    }
    let number = route(final_score as u8, effective_kind, &names[2]);
    let folder_name = app
        .settings
        .folder_names
        .get(&number.to_string())
        .context("Folder role is not configured")?;
    let root = approved(&job.output, &app.settings)?;
    let folder = root.join(folder_name);
    fs::create_dir_all(&folder)?;
    let folder = approved(&folder, &app.settings)?;
    if !within(&folder, &root) {
        bail!("Output folder resolves outside the selected root");
    }
    let extension = source
        .extension()
        .context("Missing extension")?
        .to_string_lossy()
        .to_lowercase();
    let base = format!(
        "{}_{}_{}_{}",
        names[0], names[1], names[2], final_score as u8
    );
    let mut destination = folder.join(format!("{base}.{extension}"));
    let mut suffix = 2;
    while destination.exists() {
        destination = folder.join(format!("{base}_{suffix}.{extension}"));
        suffix += 1;
    }
    item.score = Some(final_score as f64);
    item.output = Some(destination.clone());
    save_item(&app.db.lock().unwrap(), &job.id, item)?; // Persist intent before copying/publishing.
    let copy_started = std::time::Instant::now();
    item.copied = verified_copy(&source, &destination, item.source_sha.as_ref().unwrap())?;
    item.timings.insert(
        "verifiedCopySeconds".into(),
        copy_started.elapsed().as_secs_f64(),
    );
    item.timings.insert(
        "totalItemSeconds".into(),
        item_started.elapsed().as_secs_f64(),
    );
    item.state = "done".into();
    item.stage = Some("Output copy verified".into());
    Ok(())
}
fn notification_xml(title: &str, message: &str) -> String {
    let escape = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    };
    format!("<toast activationType=\"protocol\" launch=\"http://127.0.0.1:47831/#review\" duration=\"short\"><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",escape(title),escape(message))
}
fn notify(title: &str, message: &str) -> Result<()> {
    #[cfg(windows)]
    {
        use windows::{
            runtime::HSTRING,
            Data::Xml::Dom::XmlDocument,
            UI::Notifications::{ToastNotification, ToastNotificationManager},
        };
        let document =
            XmlDocument::new().map_err(|e| anyhow::anyhow!("Windows notification: {e}"))?;
        document
            .LoadXml(HSTRING::from(notification_xml(title, message)))
            .map_err(|e| anyhow::anyhow!("Windows notification XML: {e}"))?;
        let toast = ToastNotification::CreateToastNotification(document)
            .map_err(|e| anyhow::anyhow!("Windows notification: {e}"))?;
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(HSTRING::from(
            winrt_notification::Toast::POWERSHELL_APP_ID,
        ))
        .map_err(|e| anyhow::anyhow!("Windows notifier: {e}"))?;
        notifier
            .Show(&toast)
            .map_err(|e| anyhow::anyhow!("Windows notification: {e}"))?;
    }
    #[cfg(not(windows))]
    {
        let _ = (title, message);
    }
    Ok(())
}
fn runner(app: Arc<App>) {
    let mut scorer: Option<Scorer> = None;
    loop {
        let queued: Option<Job> = {
            let db = app.db.lock().unwrap();
            let id: Option<String> = db
                .query_row(
                    "SELECT id FROM jobs WHERE state='queued' ORDER BY id LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .optional()
                .unwrap_or(None);
            id.and_then(|id| {
                let mut job = load_job(&db, &id).ok()?;
                job.state = "running".into();
                job.error = None;
                save_job(&db, &job).ok()?;
                Some(job)
            })
        };
        let Some(mut job) = queued else {
            thread::sleep(Duration::from_millis(500));
            continue;
        };
        let id = job.id.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
            if scorer.is_none() && !app.settings.tools.as_ref().and_then(|t|t.visual_reviewer.as_ref()).is_some_and(|p|p.file_name().is_some_and(|n|n=="fast_visual_reviewer.py")) {
                scorer = Scorer::new(
                    &app.settings.model,
                    &app.settings.model_sha256,
                    &app.settings.runtime,
                ).ok();
            }
            let rows = items(&app.db.lock().unwrap(), &id)?;
            for mut item in rows {
                if load_job(&app.db.lock().unwrap(), &id)?.state == "cancel_requested" {
                    job.state = "cancelled".into();
                    save_job(&app.db.lock().unwrap(), &job)?;
                    return Ok(());
                }
                if item.state == "done" || (item.state=="needs_review" && item.visual_review.is_none() && !app.settings.tools.as_ref().is_some_and(evidence::automatic_configured)) {
                    continue;
                }
                item.state = "processing".into();
                item.stage=Some("Collecting existing local evidence".into());
                item.error = None;
                save_item(&app.db.lock().unwrap(), &id, &item)?;
                if let Err(error) = process_item(&app, &job, &mut item, scorer.as_mut()) {
                    item.state = "failed".into();
                    item.error = Some(format!("{error:#}"));
                }
                save_item(&app.db.lock().unwrap(), &id, &item)?;
            }
            let detail = summary(&app.db.lock().unwrap(), &job, false)?;
            job.state = if detail["failed"].as_u64().unwrap_or(0) > 0 {
                "completed_with_errors"
            } else if detail["needsReview"].as_u64().unwrap_or(0)>0 {
                "awaiting_review"
            } else {
                "completed"
            }
            .into();
            job.notification = Some("attempted".into());
            save_job(&app.db.lock().unwrap(), &job)?;
            let message=format!("{} photos finished; {} output copies verified; {} errors; {} need visual review; {} need identification. Open the local dashboard for results.",job.total,detail["completed"],detail["failed"],detail["needsReview"],detail["needsIdentification"]);
            job.notification = Some(match notify("Shoot to Post — shoot finished", &message) {
                Ok(()) => "submitted to Windows".into(),
                Err(e) => e.to_string(),
            });
            save_job(&app.db.lock().unwrap(), &job)?;
            Ok(())
        })).unwrap_or_else(|_|Err(anyhow::anyhow!("Processing component stopped unexpectedly; the saved shoot can be resumed")));
        if let Err(error) = result {
            let db = app.db.lock().unwrap();
            if let Ok(mut job) = load_job(&db, &id) {
                job.state = "paused".into();
                job.error = Some(format!("{error:#}"));
                let _ = save_job(&db, &job);
            }
        }
    }
}
fn json_response(value: Value, status: u16) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_data(serde_json::to_vec(&value).unwrap())
        .with_status_code(StatusCode(status))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
        .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
}
fn handle(app: &App, request: &mut tiny_http::Request) -> Result<Value> {
    let route = request.url().split('?').next().unwrap_or("").to_owned();
    let method = request.method().clone();
    let parts: Vec<_> = route.trim_matches('/').split('/').collect();
    if route == "/health" && method == Method::Get {
        return Ok(
            json!({"ready":true,"processingReady":app.settings.tools.as_ref().is_some_and(|t|[&t.analysis_python,&t.ocr_python,&t.technical_script,&t.registration_script,&t.collector,&t.archive_database,&app.settings.raw_decoder].iter().all(|p|p.is_file())),"engine":"rust-aviation-rubric-14","automaticVisualReviewer":app.settings.tools.as_ref().is_some_and(evidence::automatic_configured),"scoreCalibration":"deferred","fullShootAcceptance":"deferred"}),
        );
    }
    if route == "/settings" && method == Method::Get {
        return Ok(
            json!({"allowedRoots":app.settings.allowed_roots.iter().map(|p|public_path(p)).collect::<Vec<_>>(),"defaultOutput":public_path(&app.settings.default_output),"formats":["JPEG","PNG","CR3"]}),
        );
    }
    if route == "/pick-folder" && method == Method::Post {
        if app.picker_busy.swap(true, Ordering::SeqCst) {
            bail!("Folder picker is already open");
        }
        #[cfg(windows)]
        let picked = rfd::FileDialog::new()
            .set_title("Shoot to Post — choose a local folder")
            .pick_folder();
        #[cfg(not(windows))]
        let picked: Option<PathBuf> = None;
        app.picker_busy.store(false, Ordering::SeqCst);
        return Ok(
            json!({"path":picked.map(|p|approved(&p,&app.settings).map(|r|public_path(&r))).transpose()?}),
        );
    }
    if route == "/jobs" && method == Method::Post {
        let mut body = String::new();
        request.as_reader().take(16_385).read_to_string(&mut body)?;
        if body.len() > 16_384 {
            bail!("Request too large");
        }
        return create_job(app, serde_json::from_str(&body)?);
    }
    if route == "/jobs" && method == Method::Get {
        let db = app.db.lock().unwrap();
        let mut q = db.prepare(
            "SELECT data FROM jobs ORDER BY json_extract(data,'$.createdAt') DESC LIMIT 50",
        )?;
        let rows = q.query_map([], |r| r.get::<_, String>(0))?;
        let mut result = Vec::new();
        for row in rows {
            let job: Job = serde_json::from_str(&row?)?;
            result.push(summary(&db, &job, false)?);
        }
        return Ok(json!({"jobs":result}));
    }
    if parts.first() == Some(&"jobs") && parts.len() >= 2 {
        let id = parts[1];
        if !id.starts_with("shoot-") || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            bail!("Invalid job ID");
        }
        let db = app.db.lock().unwrap();
        let mut job = load_job(&db, id)?;
        if method == Method::Get && parts.len() == 2 {
            return summary(&db, &job, true);
        }
        if method == Method::Post && parts.len() == 3 && parts[2] == "review" {
            if ["queued", "running", "cancel_requested"].contains(&job.state.as_str()) {
                bail!("Wait for processing to pause before submitting a visual review");
            }
            let mut body = String::new();
            request.as_reader().take(65_537).read_to_string(&mut body)?;
            if body.len() > 65_536 {
                bail!("Review request too large");
            }
            let submission: ReviewSubmission = serde_json::from_str(&body)?;

            let mut rows = items(&db, id)?;
            let item = rows
                .iter_mut()
                .find(|r| r.ordinal == submission.ordinal)
                .context("Photo ordinal not found")?;
            if item.state == "done" || item.output.is_some() {
                bail!("Completed/planned output cannot be silently rescored");
            }
            if item.source_sha.as_deref() != Some(submission.source_sha256.as_str())
                || file_hash(&approved(&item.source, &app.settings)?)? != submission.source_sha256
            {
                bail!("Review checksum does not match the photo evidence");
            }
            if let Some(identity) = &submission.identification {
                for i in 0..3 {
                    if identity.labels[i].len() > 64
                        || identity.evidence[i].trim().is_empty()
                        || identity.evidence[i].len() > 4000
                    {
                        bail!("Identification labels need explicit supporting evidence");
                    }
                }
            }
            let kind = item.effective_kind.as_deref().unwrap_or(&job.kind);
            review::evaluate(&submission.review, kind == "aviation")?;
            item.visual_review = Some(submission.review);
            item.identification_review = submission.identification;
            item.state = "pending".into();
            item.error = None;
            save_item(&db, id, item)?;
            // Explicit Resume applies accepted reviews; merely importing never copies a file.
            job.notification = None;
            save_job(&db, &job)?;
            return summary(&db, &job, true);
        }
        if method == Method::Post && parts.len() == 3 {
            if parts[2] == "cancel" && ["running", "queued"].contains(&job.state.as_str()) {
                job.state = if job.state == "queued" {
                    "cancelled"
                } else {
                    "cancel_requested"
                }
                .into();
            } else if parts[2] == "resume"
                && [
                    "paused",
                    "cancelled",
                    "completed_with_errors",
                    "awaiting_review",
                ]
                .contains(&job.state.as_str())
            {
                if db.query_row("SELECT COUNT(*) FROM jobs WHERE state IN ('queued','running','cancel_requested')",[],|r|r.get::<_,usize>(0))?>0 {bail!("Another shoot is active");}
                job.state = "queued".into();
                job.error = None;
            } else {
                bail!("This job cannot perform that action in its current state");
            }
            save_job(&db, &job)?;
            return summary(&db, &job, false);
        }
    }
    bail!("Unknown worker operation")
}

fn open_store(path: &Path) -> Result<Connection> {
    fs::create_dir_all(path.parent().context("Missing job database parent")?)?;
    let db = Connection::open(path)?;
    db.busy_timeout(Duration::from_secs(5))?;
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY,state TEXT NOT NULL,data TEXT NOT NULL); CREATE TABLE IF NOT EXISTS items(job_id TEXT NOT NULL,ordinal INTEGER NOT NULL,state TEXT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(job_id,ordinal));CREATE INDEX IF NOT EXISTS items_source_sha ON items(json_extract(data,'$.sourceSha'));")?;
    let old: Vec<String> = {
        let mut q =
            db.prepare("SELECT data FROM jobs WHERE state IN ('running','cancel_requested')")?;
        let records = q
            .query_map([], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        records
    };
    for raw in old {
        let mut job: Job = serde_json::from_str(&raw)?;
        job.state = "paused".into();
        job.error = Some("Worker restarted; resume the saved shoot when ready".into());
        save_job(&db, &job)?;
    }
    Ok(db)
}

fn run() -> Result<()> {
    let config = std::env::args_os()
        .nth(1)
        .context("Pass the local worker configuration path")?;
    let settings: Settings = serde_json::from_slice(&fs::read(config)?)?;
    if !settings.bind.starts_with("127.0.0.1:") || settings.token.len() < 32 {
        bail!("Worker must use loopback and a strong local token");
    }
    for (number, name) in &settings.folder_names {
        if !["2", "3", "4", "5", "6"].contains(&number.as_str())
            || name.contains(['/', '\\'])
            || !name.starts_with(&(number.to_owned() + ". "))
        {
            bail!("Invalid output folder role");
        }
    }
    let server =
        Server::http(&settings.bind).map_err(|e| anyhow::anyhow!("Worker listener: {e}"))?;
    let db = open_store(&settings.database)?;
    let app = Arc::new(App {
        settings,
        db: Mutex::new(db),
        picker_busy: AtomicBool::new(false),
    });
    let runner_app = app.clone();
    thread::spawn(move || runner(runner_app));
    println!("Shoot to Post worker ready");
    for mut request in server.incoming_requests() {
        let authorized = request.headers().iter().any(|h| {
            h.field.equiv("Authorization")
                && h.value.as_str() == format!("Bearer {}", app.settings.token)
        });
        if !authorized || !request.remote_addr().is_some_and(|r| r.ip().is_loopback()) {
            let _ = request.respond(json_response(json!({"error":"Unauthorized"}), 403));
            continue;
        }
        // A native picker can remain open while status and cancellation requests continue.
        let app = app.clone();
        thread::spawn(move || {
            let response = match handle(&app, &mut request) {
                Ok(value) => json_response(value, 200),
                Err(error) => json_response(json!({"error":format!("{error:#}")}), 400),
            };
            let _ = request.respond(response);
        });
    }
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
    fn missing_visual_evidence_stays_unrated_without_output() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let output = dir.path().join("out");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&output).unwrap();
        let image = source.join("Example_A320_N123AB_99.jpg");
        fs::write(&image, b"synthetic component bytes, never visually scored").unwrap();
        let sha = file_hash(&image).unwrap();
        let settings = Settings {
            bind: "127.0.0.1:0".into(),
            token: "synthetic-only-token-000000000000000".into(),
            database: dir.path().join("jobs.sqlite"),
            allowed_roots: vec![dir.path().into()],
            model: "unused".into(),
            model_sha256: "unused".into(),
            runtime: "unused".into(),
            raw_decoder: "unused".into(),
            default_output: output.clone(),
            folder_names: BTreeMap::new(),
            tools: None,
        };
        let db = open_store(&settings.database).unwrap();
        let app = App {
            settings,
            db: Mutex::new(db),
            picker_busy: AtomicBool::new(false),
        };
        let created = create_job(
            &app,
            NewJob {
                source,
                output: output.clone(),
                kind: "aviation".into(),
                request_id: Some("needs-review-component".into()),
            },
        )
        .unwrap();
        let id = created["job"]["id"].as_str().unwrap();
        let job = load_job(&app.db.lock().unwrap(), id).unwrap();
        let mut item = items(&app.db.lock().unwrap(), id).unwrap().remove(0);
        // Cached synthetic evidence avoids running decoders/model/helpers in this component test.
        item.evidence = Some(json!({"sourceSha256":sha,"archiveIdentity":null,"errors":[]}));
        process_item(&app, &job, &mut item, None).unwrap();
        assert_eq!(item.state, "needs_review");
        assert!(item.score.is_none());
        assert!(item.output.is_none());
        assert_eq!(file_hash(&image).unwrap(), sha);
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
        save_item(&app.db.lock().unwrap(), id, &item).unwrap();
        let report = summary(&app.db.lock().unwrap(), &job, true).unwrap();
        assert_eq!(report["needsReview"], 1);
        assert_eq!(report["manifest"][0]["ratingStatus"], "unrated");
        assert_eq!(report["manifest"][0]["fileOperationStatus"], "unrated");
        assert_eq!(
            report["items"][0]["assessment"]["rubric"]["finalScore"],
            Value::Null
        );
    }
    #[test]
    fn automatic_assessment_reaches_rubric_without_manual_scoring() {
        // Contract fixture only: no photo, model inference, job, or score comparison.
        let example = include_str!("../../examples/review-submission.json")
            .replace("REPLACE:", "Synthetic contract:");
        let example: Value = serde_json::from_str(&example).unwrap();
        let mut automatic =
            json!({"assessable":true,"effectiveKind":"aviation","review":example["review"]});
        let (review, kind) = validated_automatic(&automatic).unwrap();
        let result = review::evaluate(&review, kind == "aviation").unwrap();
        assert_eq!(result["finalScore"], 91);
        assert_eq!(route(91, &kind, "Unknown"), 4);
        automatic["review"]["completeness"] = json!("cut-off");
        let (review, kind) = validated_automatic(&automatic).unwrap();
        assert_eq!(
            review::evaluate(&review, kind == "aviation").unwrap()["finalScore"],
            79
        );
        automatic["effectiveKind"] = json!("non-aviation");
        automatic["review"]["completeness"] = json!("not-applicable");
        let (review, kind) = validated_automatic(&automatic).unwrap();
        assert_eq!(
            route(
                review::evaluate(&review, false).unwrap()["finalScore"]
                    .as_u64()
                    .unwrap() as u8,
                &kind,
                "Unknown"
            ),
            5
        );
        automatic["assessable"] = json!(false);
        assert!(validated_automatic(&automatic).is_err());
    }
    #[test]
    fn accepted_cache_refuses_failed_or_mismatched_assessments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        let mut automatic = json!({"assessable":true,"accepted":true,"modelId":"synthetic cache fixture","reviewedAt":"2026-11-22T00:00:00Z","viewContext":{"nativeCrops":[{}]},"compactObservation":{"k":1,"c":1,"m":1,"z":[4,4,4,4,4],"q":[6,6,6,5,5,5],"d":[]}});
        let value = json!({"key":"versioned-key","sourceSha256":"exact-bytes","automatic":automatic,"evidence":{"sourceSha256":"exact-bytes"}});
        save_accepted_cache(&path, &value).unwrap();
        assert!(accepted_cache(&path, "versioned-key", "exact-bytes").is_some());
        assert!(accepted_cache(&path, "changed-policy", "exact-bytes").is_none());
        assert!(accepted_cache(&path, "versioned-key", "changed-bytes").is_none());
        automatic["accepted"] = json!(false);
        fs::write(&path,serde_json::to_vec(&json!({"key":"versioned-key","sourceSha256":"exact-bytes","automatic":automatic,"evidence":{"sourceSha256":"exact-bytes"}})).unwrap()).unwrap();
        assert!(accepted_cache(&path, "versioned-key", "exact-bytes").is_none());
    }
    #[test]
    fn routing_boundaries_and_labels() {
        assert_eq!(route(92, "aviation", "Unknown"), 2);
        assert_eq!(route(91, "aviation", "Unknown"), 4);
        assert_eq!(route(80, "aviation", "N123AB"), 3);
        assert_eq!(route(79, "aviation", "N123AB"), 6);
        assert_eq!(route(99, "non-aviation", "N123AB"), 5);
        assert_eq!(
            labels(Path::new("Airline_B787_N123AB_97.jpg")),
            ["Airline", "B787", "N123AB"]
        );
        assert_eq!(
            labels(Path::new("IMG_1234.CR3")),
            ["Unknown", "Unknown", "Unknown"]
        );
    }
    #[test]
    fn verified_copy_never_overwrites_and_checks_source() {
        let folder = tempfile::tempdir().unwrap();
        let source = folder.path().join("source");
        let out = folder.path().join("out");
        fs::write(&source, b"original").unwrap();
        let sha = file_hash(&source).unwrap();
        assert!(verified_copy(&source, &out, &sha).unwrap());
        assert!(!verified_copy(&source, &out, &sha).unwrap());
        fs::write(&out, b"other").unwrap();
        assert!(verified_copy(&source, &out, &sha).is_err());
        assert_eq!(fs::read(&out).unwrap(), b"other");
        assert_eq!(fs::read(&source).unwrap(), b"original");
        assert!(verified_copy(&source, &folder.path().join("bad"), "bad").is_err());
    }
    #[test]
    fn persisted_job_recovers_without_redoing_finished_items() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("input");
        let output = root.path().join("output");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&output).unwrap();
        fs::write(source.join("first.jpg"), b"component fixture, never scored").unwrap();
        fs::write(
            source.join("second.cr3"),
            b"component fixture, never decoded",
        )
        .unwrap();
        fs::write(source.join("notes.txt"), b"sidecar").unwrap();
        let database = root.path().join("jobs.sqlite");
        let settings = Settings {
            bind: "127.0.0.1:0".into(),
            token: "local-component-test-token-only-00000".into(),
            database: database.clone(),
            allowed_roots: vec![root.path().into()],
            model: "unused".into(),
            model_sha256: "unused".into(),
            runtime: "unused".into(),
            raw_decoder: "unused".into(),
            default_output: output.clone(),
            folder_names: BTreeMap::new(),
            tools: None,
        };
        let app = App {
            settings,
            db: Mutex::new(open_store(&database).unwrap()),
            picker_busy: AtomicBool::new(false),
        };
        let created = create_job(
            &app,
            NewJob {
                source: source.clone(),
                output: output.clone(),
                kind: "aviation".into(),
                request_id: Some("component-retry".into()),
            },
        )
        .unwrap();
        let id = created["job"]["id"].as_str().unwrap().to_owned();
        assert_eq!(created["job"]["total"], 2);
        assert_eq!(created["job"]["unsupported"], 1);
        let repeated = create_job(
            &app,
            NewJob {
                source: source.clone(),
                output: output.clone(),
                kind: "aviation".into(),
                request_id: Some("component-retry".into()),
            },
        )
        .unwrap();
        assert_eq!(repeated["job"]["id"], created["job"]["id"]);
        assert!(create_job(
            &app,
            NewJob {
                source: source.clone(),
                output: output.clone(),
                kind: "aviation".into(),
                request_id: None,
            }
        )
        .is_err());
        {
            let db = app.db.lock().unwrap();
            let mut job = load_job(&db, &id).unwrap();
            job.state = "running".into();
            save_job(&db, &job).unwrap();
            let mut rows = items(&db, &id).unwrap();
            rows[0].state = "done".into();
            save_item(&db, &id, &rows[0]).unwrap();
        }
        drop(app);
        let db = open_store(&database).unwrap();
        assert_eq!(load_job(&db, &id).unwrap().state, "paused");
        let rows = items(&db, &id).unwrap();
        assert_eq!(rows[0].state, "done");
        assert_eq!(rows[1].state, "pending");
        assert_eq!(
            fs::read(source.join("first.jpg")).unwrap(),
            b"component fixture, never scored"
        );
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
    }
}
