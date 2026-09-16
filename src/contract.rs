use serde::{Deserialize, Serialize};

/// The primary structured response payload returned by `servo-browse` for AI agent consumption.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentResponse {
    pub meta: PageMetadata,
    pub tui_render: String,
    pub markdown_render: String,
    pub links: Vec<IndexedLink>,
    pub ads: Vec<DetectedAd>,
}

/// Metadata describing the fetched page and viewport configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageMetadata {
    pub url: String,
    pub title: String,
    pub http_status: u16,
    pub viewport: (u32, u32),
}

/// A hyperlink extracted from the document with an assigned index for reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexedLink {
    pub index: usize,
    pub text: String,
    pub url: String,
}

/// A detected IAB ad slot with dimension classification and bounding box coordinates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetectedAd {
    pub slot_id: String,
    pub iab_format: String,
    pub dimensions: (u32, u32),
    pub bounding_box: (f32, f32, f32, f32), // (x, y, width, height)
    pub click_url: Option<String>,
    pub alt_text: Option<String>,
    pub local_crop_path: Option<String>,
}
