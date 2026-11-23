//! Explicit conservative anchors from rubric 14 bands, not trained/calibrated grades.
use crate::review::*;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub k: u8,
    pub c: u8,
    pub m: u8,
    pub z: [u8; 5],
    pub q: [u8; 6],
    pub d: Vec<[u8; 2]>,
}
const DETAIL: [u8; 9] = [0, 6, 12, 17, 21, 24, 26, 28, 30];
const FOCUS: [u8; 8] = [0, 0, 11, 15, 18, 21, 23, 25];
const EXPOSURE: [u8; 8] = [0, 0, 7, 9, 11, 13, 14, 15];
const TEN: [u8; 7] = [0, 0, 3, 6, 8, 9, 10];
const KINDS: [DefectKind; 21] = [
    DefectKind::Focus,
    DefectKind::Shake,
    DefectKind::SubjectBlur,
    DefectKind::FailedPan,
    DefectKind::HeatHaze,
    DefectKind::WindowDistortion,
    DefectKind::ResolutionLoss,
    DefectKind::ImportantClipping,
    DefectKind::BlockedShadows,
    DefectKind::Noise,
    DefectKind::Compression,
    DefectKind::Pixelation,
    DefectKind::ColorCast,
    DefectKind::GlassTint,
    DefectKind::MixedLight,
    DefectKind::Halos,
    DefectKind::WaxyDenoise,
    DefectKind::Masking,
    DefectKind::ArtificialTexture,
    DefectKind::DetailLoss,
    DefectKind::Inconsistency,
];
fn zone(value: u8) -> Result<Zone> {
    Ok(match value {
        0 => Zone::NotVisible,
        1 => Zone::Failed,
        2 => Zone::Soft,
        3 => Zone::Acceptable,
        4 => Zone::Strong,
        5 => Zone::Critical,
        _ => bail!("z: unknown visible-zone observation"),
    })
}
fn severity(value: u8) -> Result<Severity> {
    Ok(match value {
        1 => Severity::Trace,
        2 => Severity::Mild,
        3 => Severity::Moderate,
        4 => Severity::Major,
        5 => Severity::Severe,
        _ => bail!("d: unknown defect severity"),
    })
}
fn point(band: u8, table: &[u8]) -> Result<u8> {
    if band == 0 {
        bail!("q: essential category is unassessable; leave Unrated");
    }
    table
        .get(band as usize)
        .copied()
        .context("q: category band outside its rubric vocabulary")
}
pub fn to_review(automatic: &Value) -> Result<(VisualReview, String)> {
    let o: Observation = serde_json::from_value(automatic["compactObservation"].clone())?;
    let kind = match o.k {
        1 => "aviation",
        2 => "non-aviation",
        _ => bail!("k: subject classification is uncertain"),
    };
    let completeness = match (o.k, o.c) {
        (1, 1) => Completeness::Whole,
        (1, 2) => Completeness::CutOff,
        (1, 3) => Completeness::Uncertain,
        (2, 0) => Completeness::NotApplicable,
        _ => bail!("c: invalid completeness for detected subject type"),
    };
    let motion = match o.m {
        0 => Motion::AMB,
        1 => Motion::FRZ,
        2 => Motion::PAN,
        3 => Motion::FPAN,
        _ => bail!("m: invalid motion observation"),
    };
    let values = [
        point(o.q[0], &DETAIL)?,
        point(o.q[1], &FOCUS)?,
        point(o.q[2], &EXPOSURE)?,
        point(o.q[3], &TEN)?,
        point(o.q[4], &TEN)?,
        point(o.q[5], &TEN)?,
    ];
    let maxima = [30, 25, 15, 10, 10, 10];
    let labels = [
        "detail",
        "focus/motion",
        "exposure",
        "noise",
        "color",
        "processing",
    ];
    let evidence:Vec<String>=values.iter().enumerate().map(|(i,v)|format!("Model reported {} quality band {}; conservative rubric anchor {}/{}. Automatic, uncalibrated observation in the supplied views.",labels[i],o.q[i],v,maxima[i])).collect();
    if o.d.len() > 4 {
        bail!("d: too many primary consequences");
    }
    let mut defects = Vec::new();
    for [code, level] in o.d {
        let kind = *KINDS
            .get(code as usize)
            .context("d: unknown visible defect")?;
        let severity = severity(level)?;
        defects.push(Defect{kind,severity,evidence:format!("Model observed {severity:?} {kind:?} in supplied overview/native samples; physical cause not independently established")});
    }
    let primary=defects.iter().max_by_key(|d|d.severity).map(|d|d.evidence.clone()).unwrap_or_else(||"No specific defect was established by compact observation; category bands remain provisional".into());
    let review=VisualReview{rubric_version:14,reviewer:format!("{} compact-band-v1",automatic["modelId"].as_str().unwrap_or("local vision model")),reviewed_at:automatic["reviewedAt"].as_str().context("Missing observation timestamp")?.into(),viewing_evidence:serde_json::to_string(&automatic["viewContext"])?,categories:Categories{detail:values[0],focus_motion:values[1],exposure:values[2],noise:values[3],color:values[4],processing:values[5]},category_evidence:CategoryEvidence{detail:evidence[0].clone(),focus_motion:evidence[1].clone(),exposure:evidence[2].clone(),noise:evidence[3].clone(),color:evidence[4].clone(),processing:evidence[5].clone()},zones:Zones{cockpit:zone(o.z[0])?,nose:zone(o.z[1])?,midbody:zone(o.z[2])?,mechanical:zone(o.z[3])?,tail:zone(o.z[4])?},motion_mode:motion,motion_confidence:Confidence::Low,completeness,cockpit_is_target:true,close_inspection:automatic["viewContext"]["nativeCrops"].as_array().is_some_and(|v|!v.is_empty()),confidence:Confidence::Medium,uncertainty:3,strongest_attribute:"Quality-band observations are reported per category; no independent category ranking was performed".into(),primary_flaw:primary,likely_cause:"Not diagnosed by compact observations".into(),defects};
    crate::review::evaluate(&review, kind == "aviation")?;
    Ok((review, kind.into()))
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> Value {
        json!({"modelId":"synthetic contract","reviewedAt":"2026-11-22T00:00:00Z","viewContext":{"nativeCrops":[{}]},"compactObservation":{"k":1,"c":2,"m":1,"z":[4,4,4,4,4],"q":[6,6,6,5,5,5],"d":[]}})
    }
    #[test]
    fn anchors_and_framing_preserve_gates() {
        let (r, _) = to_review(&fixture()).unwrap();
        assert_eq!(r.categories.detail, 26);
        assert_eq!(crate::review::evaluate(&r, true).unwrap()["finalScore"], 79);
    }
    #[test]
    fn missing_evidence_cannot_be_filled() {
        let mut v = fixture();
        v["compactObservation"]["q"][0] = json!(0);
        assert!(to_review(&v).is_err());
        v = fixture();
        v["compactObservation"]["c"] = json!(0);
        assert!(to_review(&v).is_err());
    }
    #[test]
    fn no_confidence_shortcut_or_pan_bypass() {
        let mut v = fixture();
        v["compactObservation"]["m"] = json!(2);
        v["compactObservation"]["z"] = json!([2, 2, 2, 2, 2]);
        assert!(to_review(&v).is_err());
    }
}
