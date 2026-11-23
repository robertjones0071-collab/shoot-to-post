use anyhow::{bail, Context, Result};
use shoot_to_post_scorer::Scorer;
use std::{collections::BTreeMap, path::PathBuf};
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
            "--raw-decoder",
        ]
        .contains(&key.as_str())
        {
            bail!("Use --model FILE --model-sha256 SHA --runtime DLL --image PHOTO [--raw-decoder EXE]");
        }
        let value = args.next().context("Missing option value")?;
        if options.insert(key, value).is_some() {
            bail!("Duplicate option");
        }
    }
    let required = |key: &str| options.get(key).with_context(|| format!("Missing {key}"));
    let model = PathBuf::from(required("--model")?);
    let runtime = PathBuf::from(required("--runtime")?);
    let source = PathBuf::from(required("--image")?);
    let raw = options.get("--raw-decoder").map(PathBuf::from);
    let dump = options.get("--dump-input").map(PathBuf::from);
    let mut scorer = Scorer::new(&model, required("--model-sha256")?, &runtime)?;
    println!(
        "{}",
        scorer.score(&source, raw.as_deref(), dump.as_deref())?
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
