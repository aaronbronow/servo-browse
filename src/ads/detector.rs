use super::IabStandard;
use crate::contract::DetectedAd;

/// Detects and classifies advertising slots against standard IAB profiles.
pub struct AdDetector;

impl AdDetector {
    pub fn new() -> Self {
        Self
    }

    /// Inspect a slot's dimensions and identifier. If it matches an IAB standard, returns a `DetectedAd`.
    pub fn inspect_slot(
        &self,
        slot_id: &str,
        width: u32,
        height: u32,
        bounding_box: (f32, f32, f32, f32),
        click_url: Option<String>,
        alt_text: Option<String>,
    ) -> Option<DetectedAd> {
        let standard = IabStandard::match_dimensions(width, height)?;
        Some(DetectedAd {
            slot_id: slot_id.to_string(),
            iab_format: standard.label().to_string(),
            dimensions: (width, height),
            bounding_box,
            click_url,
            alt_text,
            local_crop_path: None,
        })
    }
}

impl Default for AdDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspect_valid_slot() {
        let detector = AdDetector::new();
        let ad = detector.inspect_slot(
            "div-gpt-ad-1",
            728,
            90,
            (0.0, 10.0, 728.0, 90.0),
            Some("https://example.com/click".to_string()),
            Some("Sponsor Banner".to_string()),
        );

        assert!(ad.is_some());
        let ad = ad.unwrap();
        assert_eq!(ad.slot_id, "div-gpt-ad-1");
        assert_eq!(ad.iab_format, "Leaderboard (728x90)");
        assert_eq!(ad.dimensions, (728, 90));
        assert_eq!(ad.click_url, Some("https://example.com/click".to_string()));
    }

    #[test]
    fn test_inspect_invalid_dimensions() {
        let detector = AdDetector::new();
        let ad = detector.inspect_slot("div-arbitrary", 500, 500, (0.0, 0.0, 500.0, 500.0), None, None);
        assert!(ad.is_none());
    }
}
