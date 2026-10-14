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
    pub bot_mitigation: Option<BotMitigation>,
}

/// Detailed diagnostic information when an anti-bot or CAPTCHA challenge is encountered.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BotMitigation {
    /// True if an active edge barrier, CAPTCHA, or challenge was detected
    pub challenge_detected: bool,
    /// Known mitigation vendor (e.g. "DataDome", "Cloudflare Turnstile", "Akamai Bot Manager", "reCAPTCHA")
    pub provider: Option<String>,
    /// Classification of the barrier
    pub challenge_type: ChallengeType,
    /// Whether ad perception was suppressed due to this barrier
    pub ads_suppressed: bool,
    /// Specific fingerprint evidence that triggered detection
    pub evidence: Vec<String>,
    /// Actionable directive for the LLM agent
    pub agent_advice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeType {
    /// Interactive challenge (puzzle, slider, checkbox, Turnstile)
    InteractiveCaptcha,
    /// Computational / JS proof-of-work challenge
    JsChallenge,
    /// IP reputation or rate-limit block (HTTP 429)
    RateLimited,
    /// Hard access block (HTTP 403)
    AccessDenied,
    /// Generic bot challenge
    GenericChallenge,
}

/// User consent preference for GDPR / Cookie Consent Management Platforms (CMP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConsentChoice {
    /// Automatically accept all cookies and grant IAB TCF / CMP consent
    OptIn,
    /// Automatically reject optional cookies / CMP consent
    OptOut,
    /// Do not intervene in CMP lifecycle
    Ignore,
}

impl Default for ConsentChoice {
    fn default() -> Self {
        ConsentChoice::OptIn
    }
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
