//! Rubric 14 arithmetic and gates. Observations must come from a named reviewer;
//! neither OpenCV metrics nor HyperIQA can manufacture category/zone evidence.
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Categories {
    pub detail: u8,
    pub focus_motion: u8,
    pub exposure: u8,
    pub noise: u8,
    pub color: u8,
    pub processing: u8,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CategoryEvidence {
    pub detail: String,
    pub focus_motion: String,
    pub exposure: String,
    pub noise: String,
    pub color: String,
    pub processing: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Zone {
    Critical,
    Strong,
    Acceptable,
    Soft,
    Failed,
    NotVisible,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Zones {
    pub cockpit: Zone,
    pub nose: Zone,
    pub midbody: Zone,
    pub mechanical: Zone,
    pub tail: Zone,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Trace,
    Mild,
    Moderate,
    Major,
    Severe,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Motion {
    FRZ,
    PAN,
    FPAN,
    AMB,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Completeness {
    Whole,
    CutOff,
    Uncertain,
    NotApplicable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DefectKind {
    Focus,
    Shake,
    SubjectBlur,
    FailedPan,
    HeatHaze,
    WindowDistortion,
    ResolutionLoss,
    ImportantClipping,
    BlockedShadows,
    Noise,
    Compression,
    Pixelation,
    ColorCast,
    GlassTint,
    MixedLight,
    Halos,
    WaxyDenoise,
    Masking,
    ArtificialTexture,
    DetailLoss,
    Inconsistency,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Defect {
    pub kind: DefectKind,
    pub severity: Severity,
    pub evidence: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisualReview {
    pub rubric_version: u8,
    pub reviewer: String,
    pub reviewed_at: String,
    pub viewing_evidence: String,
    pub categories: Categories,
    pub category_evidence: CategoryEvidence,
    pub zones: Zones,
    pub motion_mode: Motion,
    pub motion_confidence: Confidence,
    pub completeness: Completeness,
    pub cockpit_is_target: bool,
    pub close_inspection: bool,
    pub confidence: Confidence,
    pub uncertainty: u8,
    pub strongest_attribute: String,
    pub primary_flaw: String,
    pub likely_cause: String,
    pub defects: Vec<Defect>,
}
fn nonempty(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 4000 && !s.to_ascii_uppercase().contains("REPLACE:")
}
fn owner(kind: DefectKind) -> &'static str {
    use DefectKind::*;
    match kind {
        Focus | Shake | SubjectBlur | FailedPan => "focusMotion",
        HeatHaze | WindowDistortion | ResolutionLoss | DetailLoss | Inconsistency => "detail",
        ImportantClipping | BlockedShadows => "exposure",
        Noise | Compression | Pixelation => "noise",
        ColorCast | GlassTint | MixedLight => "color",
        Halos | WaxyDenoise | Masking | ArtificialTexture => "processing",
    }
}
pub fn evaluate(review: &VisualReview, aviation: bool) -> Result<Value> {
    use DefectKind::*;
    if review.rubric_version != 14 {
        bail!("A new assessment must use rubric 14");
    }
    for text in [
        &review.reviewer,
        &review.reviewed_at,
        &review.viewing_evidence,
        &review.strongest_attribute,
        &review.primary_flaw,
        &review.likely_cause,
        &review.category_evidence.detail,
        &review.category_evidence.focus_motion,
        &review.category_evidence.exposure,
        &review.category_evidence.noise,
        &review.category_evidence.color,
        &review.category_evidence.processing,
    ] {
        if !nonempty(text) {
            bail!("Reviewer, viewing evidence, diagnosis and all six category observations are required");
        }
    }
    let original = &review.categories;
    if original.detail > 30
        || original.focus_motion > 25
        || original.exposure > 15
        || original.noise > 10
        || original.color > 10
        || original.processing > 10
    {
        bail!("Category score exceeds its rubric maximum");
    }
    if review.defects.len() > 40 || review.defects.iter().any(|d| !nonempty(&d.evidence)) {
        bail!("Each defect needs visible evidence");
    }
    let min_uncertainty = match review.confidence {
        Confidence::High => 1,
        Confidence::Medium => 3,
        Confidence::Low => 5,
    };
    if review.uncertainty < min_uncertainty || review.uncertainty > 100 {
        bail!("Uncertainty range is too narrow for the stated confidence");
    }
    if review.motion_mode == Motion::AMB && review.motion_confidence == Confidence::High {
        bail!("Ambiguous motion cannot have high motion confidence");
    }
    if !aviation && review.completeness != Completeness::NotApplicable {
        bail!("Use not-applicable completeness for a non-aircraft subject");
    }
    if aviation && review.completeness == Completeness::NotApplicable {
        bail!("Aircraft completeness must be reviewed");
    }
    let zs = [
        review.zones.cockpit,
        review.zones.nose,
        review.zones.midbody,
        review.zones.mechanical,
        review.zones.tail,
    ];
    let assessable = zs.iter().filter(|&&z| z != Zone::NotVisible).count();
    if assessable == 0 {
        bail!("No visible subject zones were assessed; leave the photo Unrated");
    }
    let strong = |z: Zone| matches!(z, Zone::Strong | Zone::Critical);
    let anchor = [
        review.zones.cockpit,
        review.zones.nose,
        review.zones.midbody,
    ]
    .into_iter()
    .any(strong);
    if review.motion_mode == Motion::PAN && !anchor {
        bail!("Successful PAN requires a Strong or Critical cockpit, nose or midbody");
    }
    if review.motion_mode == Motion::FPAN && !review.defects.iter().any(|d| d.kind == FailedPan) {
        bail!("FPAN requires failed-pan evidence and severity");
    }
    if review.motion_mode != Motion::FPAN && review.defects.iter().any(|d| d.kind == FailedPan) {
        bail!("Failed-pan evidence requires FPAN motion classification");
    }
    // Repeating one consequence in multiple categories is not an independent defect ledger.
    for (i, a) in review.defects.iter().enumerate() {
        if review.defects[..i]
            .iter()
            .any(|b| b.kind == a.kind || b.evidence.trim() == a.evidence.trim())
        {
            bail!("Duplicate defect/consequence in the ledger");
        }
    }
    let mut c = original.clone();
    let mut dependencies = Vec::new();
    macro_rules! limit {($field:ident,$n:expr,$why:expr)=>{if c.$field>$n {dependencies.push(json!({"category":stringify!($field),"before":c.$field,"after":$n,"reason":$why}));c.$field=$n;}};}
    let failed = zs.iter().filter(|&&z| z == Zone::Failed).count();
    let mut gates = Vec::new();
    if failed >= 3 {
        limit!(detail, 18, "Three or more Failed zones");
        gates.push((79, "Three or more Failed zones"));
    } else if failed >= 2 {
        limit!(detail, 22, "Two Failed zones");
        gates.push((84, "Two Failed zones"));
    } else if failed == 1 {
        gates.push((89, "One Failed zone"));
    }
    if review.zones.cockpit == Zone::Failed {
        gates.push((84, "Failed cockpit"));
        if review.cockpit_is_target {
            limit!(focus_motion, 18, "Failed intended cockpit");
        }
    }
    if review.zones.cockpit == Zone::Soft {
        gates.push((89, "Soft cockpit"));
    }
    for d in &review.defects {
        match d.severity {
            Severity::Severe => gates.push((79, "Severe visible defect")),
            Severity::Major => gates.push((84, "Major visible defect")),
            Severity::Moderate => gates.push((89, "Moderate visible defect")),
            Severity::Mild => gates.push((94, "Above 94 requires Trace defects at worst")),
            Severity::Trace => {}
        }
        match d.kind {
            Halos | WaxyDenoise | Masking | ArtificialTexture => {
                limit!(processing, 8, "Visible processing damage");
                if d.severity >= Severity::Major {
                    limit!(processing, 5, "Major processing damage");
                }
            }
            ImportantClipping => {
                limit!(exposure, 12, "Clipped important highlights");
            }
            GlassTint | MixedLight => {
                limit!(color, 8, "Unresolved glass tint or mixed-light cast");
            }
            Noise if d.severity >= Severity::Moderate => {
                limit!(noise, 8, "Obvious grain at normal viewing size");
            }
            HeatHaze if d.severity >= Severity::Moderate => {
                limit!(detail, 24, "Moderate-or-worse heat haze");
            }
            WindowDistortion if d.severity >= Severity::Major => {
                limit!(detail, 21, "Major window distortion");
            }
            FailedPan => match d.severity {
                Severity::Severe => {
                    limit!(focus_motion, 11, "Severe failed pan");
                }
                Severity::Major => {
                    limit!(focus_motion, 15, "Major failed pan");
                }
                Severity::Moderate => {
                    limit!(focus_motion, 18, "Moderate failed pan");
                }
                _ => {}
            },
            _ => {}
        }
    }
    if c.focus_motion <= 11 {
        limit!(detail, 19, "Focus/motion at most 11");
    } else if c.focus_motion <= 15 {
        limit!(detail, 23, "Focus/motion at most 15");
    } else if c.focus_motion <= 18 {
        limit!(detail, 26, "Focus/motion at most 18");
    }
    let most_strong = zs.iter().filter(|&&z| strong(z)).count() * 2 > assessable;
    let cockpit_ok = review.zones.cockpit == Zone::NotVisible
        || strong(review.zones.cockpit)
        || (review.motion_mode == Motion::PAN && anchor);
    if !most_strong
        || !cockpit_ok
        || c.focus_motion < 21
        || c.exposure < 13
        || c.noise < 8
        || c.color < 8
        || c.processing < 8
    {
        gates.push((
            93,
            "Above 93 requires mostly Strong/Critical zones and a supported anchor",
        ));
    }
    let elite = review.close_inspection
        && assessable >= 3
        && zs.iter().filter(|&&z| z == Zone::Critical).count() >= 3
        && zs.iter().all(|&z| z == Zone::NotVisible || strong(z))
        && c.focus_motion >= 24
        && c.exposure >= 14
        && c.noise >= 9
        && c.color >= 9
        && c.processing >= 9;
    if !elite {
        gates.push((
            96,
            "97–100 requires close-inspection evidence and elite zones/technical control",
        ));
    }
    let raw = u16::from(c.detail)
        + u16::from(c.focus_motion)
        + u16::from(c.exposure)
        + u16::from(c.noise)
        + u16::from(c.color)
        + u16::from(c.processing);
    let technical_cap = gates.iter().map(|g| g.0).min().unwrap_or(100);
    let technical = raw.min(technical_cap);
    let framing_cap = if aviation && review.completeness == Completeness::CutOff {
        79
    } else {
        100
    };
    let final_score = technical.min(framing_cap);
    if final_score < 80
        && framing_cap == 100
        && !review.defects.iter().any(|d| d.severity >= Severity::Major)
        && failed < 2
    {
        bail!("A below-80 assessment must explain substantial visible defects or the whole-aircraft cap");
    }
    let cap = technical_cap.min(framing_cap);
    let lower = final_score.saturating_sub(u16::from(review.uncertainty));
    let upper = (final_score + u16::from(review.uncertainty))
        .min(100)
        .min(cap);
    let ledger:Vec<_>=review.defects.iter().map(|d|json!({"kind":d.kind,"severity":d.severity,"evidence":d.evidence,"owningCategory":owner(d.kind)})).collect();
    Ok(
        json!({"engine":"aviation-rubric-14-rust-v1","rubricVersion":14,"status":"rated","categoriesSubmitted":original,"categories":c,"dependencies":dependencies,
 "rawScore":raw,"technicalScore":technical,"finalScore":final_score,"technicalCeiling":technical_cap,"wholeAircraftCeiling":framing_cap,
 "gates":gates.iter().map(|(ceiling,reason)|json!({"ceiling":ceiling,"reason":reason})).collect::<Vec<_>>(),"wholeAircraftCapApplied":framing_cap<technical,
 "confidence":review.confidence,"uncertaintyRange":[lower,upper],"provisional":review.confidence!=Confidence::High,"motionMode":review.motion_mode,"motionConfidence":review.motion_confidence,"completeness":review.completeness,
 "zones":review.zones,"defectLedger":ledger,"categoryEvidence":review.category_evidence,"strongestAttribute":review.strongest_attribute,"primaryFlaw":review.primary_flaw,"likelyCause":review.likely_cause,"reviewer":review.reviewer,"reviewedAt":review.reviewed_at,"viewingEvidence":review.viewing_evidence,
 "scoreBand":match final_score {98..=100=>"Elite",95..=97=>"Portfolio",92..=94=>"Excellent keeper",88..=91=>"Strong keeper",84..=87=>"Usable keeper",80..=83=>"Borderline",75..=79=>"Below standard",68..=74=>"Significant problems",_=>"Major failure"}}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    pub fn fixture() -> VisualReview {
        serde_json::from_value(json!({"rubricVersion":14,"reviewer":"synthetic component fixture","reviewedAt":"2026-11-22T00:00:00Z","viewingEvidence":"Synthetic evidence, no real photo scored","categories":{"detail":30,"focusMotion":25,"exposure":15,"noise":10,"color":10,"processing":10},"categoryEvidence":{"detail":"Resolved test pattern","focusMotion":"Stable test edges","exposure":"Retained test tones","noise":"Clean synthetic field","color":"Neutral synthetic field","processing":"No test artifacts"},"zones":{"cockpit":"critical","nose":"critical","midbody":"critical","mechanical":"strong","tail":"strong"},"motionMode":"FRZ","motionConfidence":"high","completeness":"whole","cockpitIsTarget":true,"closeInspection":true,"confidence":"high","uncertainty":1,"strongestAttribute":"Synthetic detail","primaryFlaw":"None visible in fixture","likelyCause":"No defect diagnosed","defects":[]})).unwrap()
    }
    #[test]
    fn arithmetic_and_whole_aircraft_cap() {
        let mut r = fixture();
        r.completeness = Completeness::CutOff;
        let v = evaluate(&r, true).unwrap();
        assert_eq!(v["rawScore"], 100);
        assert_eq!(v["technicalScore"], 100);
        assert_eq!(v["finalScore"], 79);
        assert_eq!(v["uncertaintyRange"], json!([78, 79]));
    }
    #[test]
    fn failed_pan_dependencies_and_lowest_gate() {
        let mut r = fixture();
        r.motion_mode = Motion::FPAN;
        r.defects.push(Defect {
            kind: DefectKind::FailedPan,
            severity: Severity::Severe,
            evidence: "Synthetic doubled subject edges, not background streaks".into(),
        });
        let v = evaluate(&r, true).unwrap();
        assert_eq!(v["categories"]["focusMotion"], 11);
        assert_eq!(v["categories"]["detail"], 19);
        assert_eq!(v["rawScore"], 75);
        assert_eq!(v["technicalCeiling"], 79);
        assert_eq!(v["finalScore"], 75);
    }
    #[test]
    fn confidence_alone_does_not_lower_score() {
        let mut r = fixture();
        r.confidence = Confidence::Low;
        r.uncertainty = 5;
        let v = evaluate(&r, true).unwrap();
        assert_eq!(v["finalScore"], 100);
        assert_eq!(v["uncertaintyRange"], json!([95, 100]));
        r.motion_mode = Motion::AMB;
        assert!(evaluate(&r, true).is_err());
    }
    #[test]
    fn processing_and_missing_evidence() {
        let mut r = fixture();
        r.defects.push(Defect {
            kind: DefectKind::WaxyDenoise,
            severity: Severity::Major,
            evidence: "Synthetic smeared texture".into(),
        });
        let v = evaluate(&r, true).unwrap();
        assert_eq!(v["categories"]["processing"], 5);
        assert_eq!(v["finalScore"], 84);
        r.category_evidence.color.clear();
        assert!(evaluate(&r, true).is_err());
    }
    #[test]
    fn hidden_zones_are_not_failures_and_pan_needs_anchor() {
        let mut r = fixture();
        r.zones.cockpit = Zone::NotVisible;
        assert_eq!(evaluate(&r, true).unwrap()["finalScore"], 96);
        r.motion_mode = Motion::PAN;
        r.zones.nose = Zone::Soft;
        r.zones.midbody = Zone::Soft;
        assert!(evaluate(&r, true).is_err());
    }
    #[test]
    fn every_dependency_and_severity_ceiling() {
        for (kind, severity, category, expected, cap) in [
            (DefectKind::Halos, Severity::Mild, "processing", 8, 94),
            (
                DefectKind::ImportantClipping,
                Severity::Moderate,
                "exposure",
                12,
                89,
            ),
            (DefectKind::GlassTint, Severity::Mild, "color", 8, 94),
            (DefectKind::MixedLight, Severity::Mild, "color", 8, 94),
            (DefectKind::Noise, Severity::Moderate, "noise", 8, 89),
            (DefectKind::HeatHaze, Severity::Moderate, "detail", 24, 89),
            (
                DefectKind::WindowDistortion,
                Severity::Major,
                "detail",
                21,
                84,
            ),
        ] {
            let mut r = fixture();
            r.defects.push(Defect {
                kind,
                severity,
                evidence: "Independent synthetic consequence".into(),
            });
            let result = evaluate(&r, true).unwrap();
            assert_eq!(result["categories"][category], expected);
            assert_eq!(result["technicalCeiling"], cap);
        }
        for (focus, detail) in [(18, 26), (15, 23), (11, 19)] {
            let mut r = fixture();
            r.categories.focus_motion = focus;
            r.defects.push(Defect {
                kind: DefectKind::Focus,
                severity: Severity::Major,
                evidence: "Synthetic missed focus".into(),
            });
            assert_eq!(evaluate(&r, true).unwrap()["categories"]["detail"], detail);
        }
    }
    #[test]
    fn failed_zone_gates_and_duplicate_defect_rejection() {
        let mut r = fixture();
        r.zones.tail = Zone::Failed;
        r.zones.mechanical = Zone::Failed;
        let v = evaluate(&r, true).unwrap();
        assert_eq!(v["categories"]["detail"], 22);
        assert_eq!(v["finalScore"], 84);
        r.zones.cockpit = Zone::Failed;
        let v = evaluate(&r, true).unwrap();
        assert_eq!(v["categories"]["detail"], 18);
        assert_eq!(v["categories"]["focusMotion"], 18);
        assert!(v["finalScore"].as_u64().unwrap() <= 79);
        let mut r = fixture();
        let d = Defect {
            kind: DefectKind::Noise,
            severity: Severity::Mild,
            evidence: "Same visible consequence".into(),
        };
        r.defects = vec![d.clone(), d];
        assert!(evaluate(&r, true).is_err());
        r = fixture();
        r.reviewer = "REPLACE: reviewer".into();
        assert!(evaluate(&r, true).is_err());
    }
}
