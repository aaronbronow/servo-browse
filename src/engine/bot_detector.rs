//! Heuristic detector for bot challenges, CAPTCHAs, and edge security barriers.

use scraper::{Html, Selector};
use crate::contract::{BotMitigation, ChallengeType};

pub struct BotDetector;

impl BotDetector {
    pub fn new() -> Self {
        Self
    }

    /// Inspect the HTTP status, page title, and evaluated HTML DOM for known bot challenge signatures.
    pub fn inspect(&self, status: u16, title: &str, html: &str) -> Option<BotMitigation> {
        let mut evidence = Vec::new();
        let mut provider = None;
        let mut challenge_type = None;

        let lower_title = title.to_lowercase();
        let lower_html = html.to_lowercase();

        // 1. HTTP Status Code Signals
        if status == 429 {
            evidence.push("HTTP 429 Too Many Requests status received".to_string());
            challenge_type = Some(ChallengeType::RateLimited);
        } else if status == 403 {
            evidence.push("HTTP 403 Forbidden status received".to_string());
        }

        // 2. DataDome Signatures
        if lower_title.contains("bot or not?") || lower_html.contains("geo.captcha-delivery.com") || lower_html.contains("datadome") {
            provider = Some("DataDome".to_string());
            challenge_type = Some(ChallengeType::InteractiveCaptcha);
            if lower_title.contains("bot or not?") {
                evidence.push("Page title matched DataDome signature: 'Bot or Not?'".to_string());
            }
            if lower_html.contains("geo.captcha-delivery.com") {
                evidence.push("Script/iframe contains 'geo.captcha-delivery.com'".to_string());
            }
            if lower_html.contains("datadome") {
                evidence.push("References DataDome client SDK or cookie".to_string());
            }
        }

        // 3. Akamai Bot Manager / Client Reputation
        if lower_html.contains("/botornot/") || lower_html.contains("akamaicaptcha") || lower_html.contains("sec-cpt") {
            let p = provider.map(|prev| format!("{} / Akamai", prev)).unwrap_or_else(|| "Akamai Bot Manager".to_string());
            provider = Some(p);
            if challenge_type.is_none() {
                challenge_type = Some(ChallengeType::InteractiveCaptcha);
            }
            if lower_html.contains("/botornot/") {
                evidence.push("Akamai endpoint '/botOrNot/' detected".to_string());
            }
            if lower_html.contains("sec-cpt") || lower_html.contains("akamaicaptcha") {
                evidence.push("Akamai CAPTCHA challenge component present".to_string());
            }
        }

        // 4. Cloudflare Turnstile & Under Attack Mode
        if lower_title.contains("just a moment...")
            || lower_title.contains("attention required! | cloudflare")
            || lower_html.contains("challenges.cloudflare.com")
            || lower_html.contains("cf-turnstile")
            || lower_html.contains("id=\"challenge-running\"")
            || lower_html.contains("cf-browser-verification")
        {
            provider = Some("Cloudflare Turnstile".to_string());
            challenge_type = Some(ChallengeType::InteractiveCaptcha);
            if lower_title.contains("just a moment...") {
                evidence.push("Title matched Cloudflare waiting room: 'Just a moment...'".to_string());
            }
            if lower_html.contains("challenges.cloudflare.com") {
                evidence.push("Cloudflare challenges script domain referenced".to_string());
            }
            if lower_html.contains("cf-turnstile") {
                evidence.push("Cloudflare Turnstile widget container found".to_string());
            }
        }

        // 5. AWS WAF Captcha
        if lower_html.contains("token.awswaf.com") || lower_html.contains("aws-waf-captcha") {
            provider = Some("AWS WAF Captcha".to_string());
            challenge_type = Some(ChallengeType::InteractiveCaptcha);
            evidence.push("AWS WAF CAPTCHA token provider script detected".to_string());
        }

        // 6. PerimeterX / HUMAN
        if lower_html.contains("px-captcha") || lower_html.contains("perimeterx") || lower_title.contains("access to this page has been denied") {
            provider = Some("HUMAN (PerimeterX)".to_string());
            challenge_type = Some(ChallengeType::InteractiveCaptcha);
            evidence.push("PerimeterX / HUMAN challenge token signature matched".to_string());
        }

        // 7. Google reCAPTCHA / hCaptcha challenge gates
        if provider.is_none() {
            let is_gate = status == 403
                || status == 429
                || lower_title.contains("robot")
                || lower_title.contains("captcha")
                || lower_title.contains("security check")
                || lower_title.contains("verify you are human")
                || lower_title.contains("attention required")
                || lower_html.contains("please verify")
                || lower_html.contains("complete the security check")
                || lower_html.contains("unusual traffic from your computer network");

            if is_gate && (lower_html.contains("class=\"g-recaptcha")
                || lower_html.contains("class='g-recaptcha")
                || lower_html.contains("id=\"g-recaptcha")
                || lower_html.contains("google.com/recaptcha/api.js")
                || lower_html.contains("google.com/recaptcha/enterprise.js"))
            {
                provider = Some("Google reCAPTCHA".to_string());
                challenge_type = Some(ChallengeType::InteractiveCaptcha);
                evidence.push("Google reCAPTCHA challenge gate detected".to_string());
            } else if is_gate && (lower_html.contains("class=\"h-captcha")
                || lower_html.contains("class='h-captcha")
                || lower_html.contains("id=\"h-captcha")
                || lower_html.contains("hcaptcha.com/1/api.js"))
            {
                provider = Some("hCaptcha".to_string());
                challenge_type = Some(ChallengeType::InteractiveCaptcha);
                evidence.push("hCaptcha challenge gate detected".to_string());
            }
        }

        // 8. Generic Challenge phrases in Title / Headings
        if provider.is_none() {
            let document = Html::parse_document(html);
            let h1_selector = Selector::parse("h1, h2, #challenge-stage").unwrap();
            for heading in document.select(&h1_selector) {
                let text = heading.text().collect::<Vec<_>>().join(" ").to_lowercase();
                if text.contains("verify you are human")
                    || text.contains("confirm you are human")
                    || text.contains("please verify you are a human")
                    || text.contains("robot check")
                    || text.contains("show us your human side")
                {
                    provider = Some("Generic Anti-Bot Gate".to_string());
                    challenge_type = Some(ChallengeType::InteractiveCaptcha);
                    evidence.push(format!("Header text matched challenge: '{}'", text.trim()));
                    break;
                }
            }
        }

        // Fallback for 403 / 429 if no specific provider matched
        if provider.is_none() {
            if status == 429 {
                provider = Some("Server Rate Limiter".to_string());
                challenge_type = Some(ChallengeType::RateLimited);
            } else if status == 403 && (lower_html.contains("access denied") || lower_html.contains("forbidden")) {
                provider = Some("Edge Security Policy".to_string());
                challenge_type = Some(ChallengeType::AccessDenied);
            }
        }

        if evidence.is_empty() {
            return None;
        }

        let prov = provider.unwrap_or_else(|| "Unknown Provider".to_string());
        let c_type = challenge_type.unwrap_or(ChallengeType::GenericChallenge);

        Some(BotMitigation {
            challenge_detected: true,
            provider: Some(prov.clone()),
            challenge_type: c_type,
            ads_suppressed: true,
            evidence,
            agent_advice: format!(
                "Target content and ad auctions were suppressed by {} challenge. Do not treat this page text as the actual destination or conclude that the site has no ads.",
                prov
            ),
        })
    }
}

impl Default for BotDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_datadome_challenge() {
        let detector = BotDetector::new();
        let res = detector.inspect(
            200,
            "Bot or Not?",
            "<html><body><h2>Show us your human side...</h2><script src='https://geo.captcha-delivery.com/captcha'></script></body></html>",
        );
        assert!(res.is_some());
        let mitigation = res.unwrap();
        assert!(mitigation.challenge_detected);
        assert_eq!(mitigation.provider.as_deref(), Some("DataDome"));
        assert_eq!(mitigation.challenge_type, ChallengeType::InteractiveCaptcha);
        assert!(mitigation.ads_suppressed);
    }

    #[test]
    fn test_detect_cloudflare_turnstile() {
        let detector = BotDetector::new();
        let res = detector.inspect(
            403,
            "Just a moment...",
            "<html><body><div id='challenge-running'>Checking your browser</div><script src='https://challenges.cloudflare.com/turnstile/v0/api.js'></script></body></html>",
        );
        assert!(res.is_some());
        let mitigation = res.unwrap();
        assert_eq!(mitigation.provider.as_deref(), Some("Cloudflare Turnstile"));
    }

    #[test]
    fn test_clean_page_no_mitigation() {
        let detector = BotDetector::new();
        let res = detector.inspect(
            200,
            "Sample Page",
            "<html><body><h1>Welcome</h1><p>Regular page content</p></body></html>",
        );
        assert!(res.is_none());
    }
}
