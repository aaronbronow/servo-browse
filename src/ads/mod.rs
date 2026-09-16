pub mod detector;

/// Standard Interactive Advertising Bureau (IAB) display unit dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IabStandard {
    /// 728 x 90 Leaderboard (top header / above navigation)
    Leaderboard,
    /// 300 x 250 Medium Rectangle (MPU) (sidebar / in-article stream)
    MediumRectangle,
    /// 336 x 280 Large Rectangle (content breaks / sidebars)
    LargeRectangle,
    /// 300 x 600 Half Page / Large Skyscraper (sticky right rail)
    HalfPage,
    /// 160 x 600 Wide Skyscraper (tall narrow sidebar)
    WideSkyscraper,
    /// 970 x 250 Billboard (premium top takeover)
    Billboard,
    /// 320 x 50 Mobile Leaderboard / Banner (mobile headers/footers)
    MobileBanner,
    /// 320 x 100 Large Mobile Banner
    LargeMobileBanner,
}

impl IabStandard {
    /// Match a pair of (width, height) pixel dimensions against known IAB profiles.
    pub fn match_dimensions(width: u32, height: u32) -> Option<Self> {
        match (width, height) {
            (728, 90) => Some(Self::Leaderboard),
            (300, 250) => Some(Self::MediumRectangle),
            (336, 280) => Some(Self::LargeRectangle),
            (300, 600) => Some(Self::HalfPage),
            (160, 600) => Some(Self::WideSkyscraper),
            (970, 250) => Some(Self::Billboard),
            (320, 50) => Some(Self::MobileBanner),
            (320, 100) => Some(Self::LargeMobileBanner),
            _ => None,
        }
    }

    /// Human-readable name and dimension string.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Leaderboard => "Leaderboard (728x90)",
            Self::MediumRectangle => "Medium Rectangle (300x250)",
            Self::LargeRectangle => "Large Rectangle (336x280)",
            Self::HalfPage => "Half-Page (300x600)",
            Self::WideSkyscraper => "Wide Skyscraper (160x600)",
            Self::Billboard => "Billboard (970x250)",
            Self::MobileBanner => "Mobile Banner (320x50)",
            Self::LargeMobileBanner => "Large Mobile Banner (320x100)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iab_dimension_matching() {
        assert_eq!(IabStandard::match_dimensions(728, 90), Some(IabStandard::Leaderboard));
        assert_eq!(IabStandard::match_dimensions(300, 250), Some(IabStandard::MediumRectangle));
        assert_eq!(IabStandard::match_dimensions(970, 250), Some(IabStandard::Billboard));
        assert_eq!(IabStandard::match_dimensions(123, 456), None);
    }
}
