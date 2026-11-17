use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn default_fallback_strategy() -> String {
    "manual".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeckParserProfile {
    pub engine_name: String,
    pub slide_selector: String,
    #[serde(default)]
    pub active_class: String,
    #[serde(default)]
    pub title_selector: String,
    #[serde(default)]
    pub notes_selector: String,
    #[serde(default = "default_fallback_strategy")]
    pub fallback_strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParserSlidePreview {
    pub slide_index: usize,
    pub title: String,
    pub notes: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParserTestResult {
    pub total_slides: usize,
    pub slides: Vec<ParserSlidePreview>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TakeCard {
    pub take_id: String,
    pub slide_id: String,
    pub filename: String,
    pub relative_path: String,
    pub duration_sec: f64,
    pub created_at: String,
    pub note: String,
    pub is_golden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlideTakeGroup {
    pub slide_id: String,
    pub slide_title: String,
    pub golden_take_id: Option<String>,
    #[serde(default)]
    pub next_take_number: u64,
    pub takes: Vec<TakeCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PresentationManifest {
    pub schema_version: String,
    pub presentation_id: String,
    pub transition_delay_ms: u32,
    pub slides: BTreeMap<String, SlideTakeGroup>,
}

impl PresentationManifest {
    pub fn new(presentation_id: impl Into<String>) -> Self {
        Self {
            schema_version: "1.0.0".to_string(),
            presentation_id: presentation_id.into(),
            transition_delay_ms: 400,
            slides: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveSlideInfo {
    pub slide_id: String,
    pub slide_index: usize,
    pub total_slides: usize,
    pub slide_title: String,
}

#[derive(Debug, Deserialize)]
pub struct NoteInput {
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeGroupResponse {
    pub slide_id: String,
    pub slide_title: String,
    pub golden_take_id: Option<String>,
    pub takes: Vec<TakeCard>,
}

impl From<SlideTakeGroup> for TakeGroupResponse {
    fn from(group: SlideTakeGroup) -> Self {
        Self {
            slide_id: group.slide_id,
            slide_title: group.slide_title,
            golden_take_id: group.golden_take_id,
            takes: group.takes,
        }
    }
}
