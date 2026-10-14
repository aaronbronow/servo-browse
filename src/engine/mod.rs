pub mod bot_detector;
pub mod servo_backend;

use std::path::Path;
use image::RgbaImage;
use scraper::{ElementRef, Html, Selector};
use servo::{DeviceIntPoint, DeviceIntRect};
use url::Url;
use crate::ads::detector::AdDetector;
use crate::contract::{AgentResponse, ConsentChoice, DetectedAd, IndexedLink, PageMetadata};
use crate::engine::bot_detector::BotDetector;
use crate::formatter::lynx::LynxFormatter;

pub struct HeadlessEngine {
    detector: AdDetector,
    bot_detector: BotDetector,
}

impl HeadlessEngine {
    pub fn new() -> Self {
        Self {
            detector: AdDetector::new(),
            bot_detector: BotDetector::new(),
        }
    }

    /// Run full headless browsing session using in-process Servo (SpiderMonkey + Software WebRender)
    pub fn browse_servo(
        &self,
        target_url: &str,
        cols: usize,
        selector_filter: Option<&str>,
        detect_ads: bool,
        timeout: std::time::Duration,
        capture_png: bool,
        crop_ads: bool,
        ads_dir: Option<&str>,
        consent: ConsentChoice,
    ) -> Result<(AgentResponse, Option<RgbaImage>), String> {
        let resolved_url = if target_url.starts_with("http://")
            || target_url.starts_with("https://")
            || target_url.starts_with("file://")
        {
            target_url.to_string()
        } else {
            let path = Path::new(target_url);
            if !path.exists() {
                return Err(format!("File does not exist: {}", target_url));
            }
            let abs_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            format!("file://{}", abs_path.display())
        };

        eprintln!("[servo-browse] Launching in-process Servo engine...");
        let width = ((cols * 10).max(1024)) as u32;
        let height = 800;

        let mut session = servo_backend::ServoSession::new(&resolved_url, width, height, consent)
            .map_err(|e| format!("Failed to initialize Servo session: {}", e))?;

        eprintln!("[servo-browse] Loading: {}", resolved_url);
        session.wait_for_load(timeout);

        // Allow async layout ticks and ad auction scripts to finalize
        eprintln!("[servo-browse] Evaluating dynamic DOM & ad lifecycle events...");
        let extra_start = std::time::Instant::now();
        session.spin_until(std::time::Duration::from_millis(1500), || {
            extra_start.elapsed() > std::time::Duration::from_millis(1200)
        });

        let html = session
            .get_evaluated_html(std::time::Duration::from_secs(3))
            .unwrap_or_else(|_| String::new());

        let live_ads = if detect_ads {
            session.extract_ad_inventory(std::time::Duration::from_secs(2))
        } else {
            Vec::new()
        };

        let page_title = session.delegate.page_title.borrow().clone();

        let mut response = if !html.is_empty() {
            self.process_page(&html, &resolved_url, 200, cols, selector_filter, detect_ads)
        } else {
            AgentResponse {
                meta: PageMetadata {
                    url: resolved_url.clone(),
                    title: page_title.clone().unwrap_or_else(|| resolved_url.clone()),
                    http_status: 200,
                    viewport: (width, height),
                    bot_mitigation: None,
                },
                tui_render: String::new(),
                markdown_render: String::new(),
                links: Vec::new(),
                ads: Vec::new(),
            }
        };

        if let Some(t) = page_title {
            if response.meta.title == "Untitled Document" || response.meta.title == resolved_url {
                response.meta.title = t;
            }
        }

        // Merge live ad perceptions from intercepted events into response.ads
        for live_ad in live_ads {
            if let Some(existing) = response.ads.iter_mut().find(|a| a.slot_id == live_ad.slot_id) {
                if live_ad.click_url.is_some() {
                    existing.click_url = live_ad.click_url;
                }
                if live_ad.alt_text.is_some() {
                    existing.alt_text = live_ad.alt_text;
                }
                existing.bounding_box = live_ad.bounding_box;
                existing.dimensions = live_ad.dimensions;
                existing.iab_format = live_ad.iab_format;
            } else {
                response.ads.push(live_ad);
            }
        }

        // Optional visual cropping of each detected ad creative
        if crop_ads {
            let out_dir = ads_dir.unwrap_or(".agent/artifacts/ads");
            let _ = std::fs::create_dir_all(out_dir);

            for ad in &mut response.ads {
                let (x, y, w, h) = ad.bounding_box;
                if w >= 20.0 && h >= 20.0 {
                    let rect = DeviceIntRect::new(
                        DeviceIntPoint::new(x.max(0.0) as i32, y.max(0.0) as i32),
                        DeviceIntPoint::new((x.max(0.0) + w) as i32, (y.max(0.0) + h) as i32),
                    );
                    if let Ok(cropped_img) = session.capture_crop(rect, std::time::Duration::from_secs(3)) {
                        let safe_id = ad.slot_id
                            .chars()
                            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
                            .collect::<String>();
                        let file_path = format!("{}/{}_{}x{}.png", out_dir, safe_id, ad.dimensions.0, ad.dimensions.1);
                        if cropped_img.save(&file_path).is_ok() {
                            eprintln!("[servo-browse] Cropped ad creative to: {}", file_path);
                            ad.local_crop_path = Some(file_path);
                        }
                    }
                }
            }
        }

        let screenshot = if capture_png {
            eprintln!("[servo-browse] Capturing software offscreen screenshot...");
            session.capture_screenshot(std::time::Duration::from_secs(5)).ok()
        } else {
            None
        };

        Ok((response, screenshot))
    }

    /// Load HTML content from a URL, local file path, or file:// URI.
    pub async fn load_html(&self, target: &str) -> Result<(String, String, u16), String> {
        let (html, base_url, status) = if target.starts_with("http://") || target.starts_with("https://") {
            let client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36")
                .build()
                .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

            let resp = client
                .get(target)
                .send()
                .await
                .map_err(|e| format!("Network request failed: {}", e))?;

            let final_url = resp.url().to_string();
            let status = resp.status().as_u16();
            let body = resp
                .text()
                .await
                .map_err(|e| format!("Failed to read response body: {}", e))?;

            (body, final_url, status)
        } else {
            let file_path = if let Some(stripped) = target.strip_prefix("file://") {
                stripped
            } else {
                target
            };

            let path = Path::new(file_path);
            if !path.exists() {
                return Err(format!("File does not exist: {}", file_path));
            }

            let body = tokio::fs::read_to_string(path)
                .await
                .map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;

            (body, format!("file://{}", path.canonicalize().unwrap_or_else(|_| path.to_path_buf()).display()), 200)
        };

        Ok((html, base_url, status))
    }

    /// Parse HTML, extract real text content with indexed links, detect IAB ads, and format.
    pub fn process_page(
        &self,
        html_str: &str,
        page_url: &str,
        http_status: u16,
        cols: usize,
        selector_filter: Option<&str>,
        detect_ads: bool,
    ) -> AgentResponse {
        let document = Html::parse_document(html_str);
        let base_url = Url::parse(page_url).ok();

        // 1. Extract Page Title
        let title = {
            let title_selector = Selector::parse("title").unwrap();
            let h1_selector = Selector::parse("h1").unwrap();

            if let Some(title_el) = document.select(&title_selector).next() {
                let t = title_el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if !t.is_empty() { t } else { "Untitled Document".to_string() }
            } else if let Some(h1_el) = document.select(&h1_selector).next() {
                let t = h1_el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if !t.is_empty() { t } else { "Untitled Document".to_string() }
            } else {
                page_url.to_string()
            }
        };

        let mut indexed_links: Vec<IndexedLink> = Vec::new();
        let mut detected_ads: Vec<DetectedAd> = Vec::new();
        let mut link_counter: usize = 1;

        // Helper to resolve URLs
        let resolve_url = |href: &str| -> String {
            if let Some(ref base) = base_url {
                base.join(href).map(|u| u.to_string()).unwrap_or_else(|_| href.to_string())
            } else {
                href.to_string()
            }
        };

        // 2. Detect IAB Ads if enabled
        if detect_ads {
            let ad_selector = Selector::parse(".ad-slot, [data-iab], [id*='leaderboard'], [id*='med-rect'], [id*='banner'], [id*='ad'], [class*='ad-'], [data-concert], [data-native-ad-id]").unwrap();
            for element in document.select(&ad_selector) {
                let slot_id = element.value().attr("id")
                    .or_else(|| element.value().attr("data-concert"))
                    .or_else(|| element.value().attr("data-native-ad-id"))
                    .unwrap_or("ad-slot");

                // Parse dimensions from data-iab (e.g. "728x90") or style
                let mut dimensions = None;
                if let Some(data_iab) = element.value().attr("data-iab") {
                    let parts: Vec<&str> = data_iab.split('x').collect();
                    if parts.len() == 2 {
                        if let (Ok(w), Ok(h)) = (parts[0].trim().parse::<u32>(), parts[1].trim().parse::<u32>()) {
                            dimensions = Some((w, h));
                        }
                    }
                }

                if dimensions.is_none() {
                    if let Some(style) = element.value().attr("style") {
                        let w = extract_style_px(style, "width");
                        let h = extract_style_px(style, "height");
                        if let (Some(w), Some(h)) = (w, h) {
                            dimensions = Some((w, h));
                        }
                    }
                }

                if dimensions.is_none() {
                    if let Some(concert_type) = element.value().attr("data-concert") {
                        if concert_type.contains("desktop") {
                            dimensions = Some((1200, 100));
                        } else if concert_type.contains("mobile") {
                            dimensions = Some((320, 50));
                        }
                    }
                }

                if let Some((w, h)) = dimensions {
                    let a_selector = Selector::parse("a").unwrap();
                    let img_selector = Selector::parse("img").unwrap();

                    let click_url = element.select(&a_selector).next().and_then(|a| a.value().attr("href")).map(|h| resolve_url(h));
                    let alt_text = element.select(&img_selector).next().and_then(|img| img.value().attr("alt")).map(|s| s.to_string());

                    if let Some(ad) = self.detector.inspect_slot(
                        slot_id,
                        w,
                        h,
                        (0.0, 0.0, w as f32, h as f32),
                        click_url,
                        alt_text,
                    ) {
                        detected_ads.push(ad);
                    }
                }
            }

            // Also inspect in-page script tags for googletag.defineSlot declarations
            let script_selector = Selector::parse("script").unwrap();
            let re_size = regex::Regex::new(r"\[\s*(\d+)\s*,\s*(\d+)\s*\]").unwrap();
            if let Ok(re_slot) = regex::Regex::new(r#"defineSlot\s*\(\s*["']([^"']+)["']\s*,\s*([^,]+|\[(?:[^\[\]]*|\[[^\[\]]*\])*\])\s*,\s*["']([^"']+)["']\s*\)"#) {
                for script_el in document.select(&script_selector) {
                    let script_text = script_el.text().collect::<Vec<_>>().join(" ");
                    for cap in re_slot.captures_iter(&script_text) {
                        let slot_id = cap.get(3).map(|m| m.as_str()).unwrap_or("gpt-slot");
                        let size_raw = cap.get(2).map(|m| m.as_str()).unwrap_or("");
                        let (w, h) = if let Some(size_cap) = re_size.captures(size_raw) {
                            (
                                size_cap[1].parse::<u32>().unwrap_or(300),
                                size_cap[2].parse::<u32>().unwrap_or(250),
                            )
                        } else {
                            (300, 250)
                        };
                        if !detected_ads.iter().any(|a| a.slot_id == slot_id) {
                            if let Some(ad) = self.detector.inspect_slot(
                                slot_id,
                                w,
                                h,
                                (0.0, 0.0, w as f32, h as f32),
                                None,
                                None,
                            ) {
                                detected_ads.push(ad);
                            }
                        }
                    }
                }
            }
        }

        // 3. Extract Document Content
        let mut body_lines: Vec<String> = Vec::new();

        // Scope to user selector or body/main
        let root_elements = if let Some(sel) = selector_filter {
            if let Ok(parsed_sel) = Selector::parse(sel) {
                document.select(&parsed_sel).collect::<Vec<_>>()
            } else {
                let body_sel = Selector::parse("body").unwrap();
                document.select(&body_sel).collect::<Vec<_>>()
            }
        } else {
            let body_sel = Selector::parse("body").unwrap();
            document.select(&body_sel).collect::<Vec<_>>()
        };

        let block_selector = Selector::parse("h1, h2, h3, h4, h5, h6, p, li, blockquote, div.ad-slot, div[data-iab], div[id*='ad'], div[id*='banner']").unwrap();

        for root in root_elements {
            for block in root.select(&block_selector) {
                // Skip cookie consent / CMP banners from text extraction
                let is_cmp = {
                    let check_el = |val: &scraper::node::Element| -> bool {
                        if let Some(id) = val.attr("id") {
                            let id_lower = id.to_lowercase();
                            if id_lower.contains("onetrust")
                                || id_lower.contains("sp_message")
                                || id_lower.contains("cookie-banner")
                                || id_lower.contains("cookie-consent")
                                || id_lower.contains("didomi")
                            {
                                return true;
                            }
                        }
                        if let Some(classes) = val.attr("class") {
                            let class_lower = classes.to_lowercase();
                            if class_lower.contains("onetrust")
                                || class_lower.contains("fc-consent")
                                || class_lower.contains("qc-cmp2")
                                || class_lower.contains("cookie-banner")
                                || class_lower.contains("cookie-consent")
                                || class_lower.contains("cookie-notice")
                            {
                                return true;
                            }
                        }
                        false
                    };

                    if check_el(block.value()) {
                        true
                    } else {
                        let mut matched = false;
                        for ancestor in block.ancestors() {
                            if let Some(elem) = ElementRef::wrap(ancestor) {
                                if check_el(elem.value()) {
                                    matched = true;
                                    break;
                                }
                            }
                        }
                        matched
                    }
                };
                if is_cmp {
                    continue;
                }

                let tag_name = block.value().name();

                // Check if this block is an ad slot
                let slot_id = block.value().attr("id").unwrap_or("");
                if !slot_id.is_empty() {
                    if let Some(ad) = detected_ads.iter().find(|a| a.slot_id == slot_id) {
                        let link_ref = if let Some(ref click_url) = ad.click_url {
                            let idx = link_counter;
                            link_counter += 1;
                            indexed_links.push(IndexedLink {
                                index: idx,
                                text: ad.alt_text.clone().unwrap_or_else(|| ad.iab_format.clone()),
                                url: click_url.clone(),
                            });
                            format!(" [{}]", idx)
                        } else {
                            String::new()
                        };

                        body_lines.push(format!(
                            "[AD: {} -> {}{}]",
                            ad.iab_format,
                            ad.alt_text.as_deref().unwrap_or(&ad.slot_id),
                            link_ref
                        ));
                        body_lines.push(String::new());
                        continue;
                    }
                }

                // Normal text block: extract text and inline links
                let mut block_text = String::new();
                for child in block.children() {
                    if let Some(text_node) = child.value().as_text() {
                        block_text.push_str(text_node);
                    } else if let Some(element) = child.value().as_element() {
                        if element.name() == "a" {
                            let anchor_text = scraper::ElementRef::wrap(child)
                                .map(|el| el.text().collect::<Vec<_>>().join(" "))
                                .unwrap_or_default();

                            let href = element.attr("href").unwrap_or("");
                            if !href.is_empty() {
                                let idx = link_counter;
                                link_counter += 1;
                                indexed_links.push(IndexedLink {
                                    index: idx,
                                    text: anchor_text.trim().to_string(),
                                    url: resolve_url(href),
                                });
                                block_text.push_str(&format!("{} [{}]", anchor_text.trim(), idx));
                            } else {
                                block_text.push_str(&anchor_text);
                            }
                        } else if let Some(el_ref) = scraper::ElementRef::wrap(child) {
                            block_text.push_str(&el_ref.text().collect::<Vec<_>>().join(" "));
                        }
                    }
                }

                let cleaned = block_text.split_whitespace().collect::<Vec<_>>().join(" ");
                if !cleaned.is_empty() {
                    match tag_name {
                        "h1" => {
                            body_lines.push(format!("# {}", cleaned));
                            body_lines.push(String::new());
                        }
                        "h2" => {
                            body_lines.push(format!("## {}", cleaned));
                            body_lines.push(String::new());
                        }
                        "h3" => {
                            body_lines.push(format!("### {}", cleaned));
                            body_lines.push(String::new());
                        }
                        "li" => {
                            body_lines.push(format!("  * {}", cleaned));
                        }
                        _ => {
                            body_lines.push(cleaned);
                            body_lines.push(String::new());
                        }
                    }
                }
            }
        }

        let body_content = body_lines.join("\n");

        // 4. Inspect for anti-bot / CAPTCHA barriers
        let bot_mitigation = self.bot_detector.inspect(http_status, &title, html_str);

        // 5. Format into Lynx TUI representation
        let formatter = LynxFormatter::new(cols);
        let tui_render = formatter.format(&title, &body_content, &indexed_links, bot_mitigation.as_ref());

        // Markdown representation
        let mut md = format!("# {}\n\n{}", title, body_content);
        if !indexed_links.is_empty() {
            md.push_str("\n\n---\n");
            for link in &indexed_links {
                md.push_str(&format!("[{}]: {}\n", link.index, link.url));
            }
        }

        AgentResponse {
            meta: PageMetadata {
                url: page_url.to_string(),
                title,
                http_status,
                viewport: (cols as u32, 24),
                bot_mitigation,
            },
            tui_render,
            markdown_render: md,
            links: indexed_links,
            ads: detected_ads,
        }
    }
}

fn extract_style_px(style: &str, property: &str) -> Option<u32> {
    for decl in style.split(';') {
        let parts: Vec<&str> = decl.split(':').collect();
        if parts.len() == 2 && parts[0].trim().eq_ignore_ascii_case(property) {
            let val = parts[1].trim().trim_end_matches("px").trim();
            if let Ok(num) = val.parse::<u32>() {
                return Some(num);
            }
        }
    }
    None
}

impl Default for HeadlessEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_page_with_links_and_ads() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test Page</title></head>
            <body>
                <h1>Heading 1</h1>
                <div id="leaderboard" class="ad-slot" data-iab="728x90">
                    <a href="https://example.com/ad-click"><img alt="Ad Sponsor" /></a>
                </div>
                <p>Hello world with a <a href="https://example.com/learn">link</a>.</p>
            </body>
            </html>
        "#;

        let engine = HeadlessEngine::new();
        let resp = engine.process_page(html, "https://example.com", 200, 80, None, true);

        assert_eq!(resp.meta.title, "Test Page");
        assert_eq!(resp.meta.http_status, 200);
        assert_eq!(resp.ads.len(), 1);
        assert_eq!(resp.ads[0].iab_format, "Leaderboard (728x90)");
        assert_eq!(resp.links.len(), 2);
        assert!(resp.tui_render.contains("Heading 1"));
        assert!(resp.tui_render.contains("[AD: Leaderboard (728x90)"));
        assert!(resp.tui_render.contains("── References"));
    }

    #[test]
    fn test_process_page_strips_cmp_cookie_banner() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Clean Content</title></head>
            <body>
                <div id="onetrust-banner-sdk" class="ot-sdk-container">
                    <p>By clicking "Accept All Cookies", you agree to the storing of cookies...</p>
                    <button id="onetrust-accept-btn-handler">Accept All Cookies</button>
                </div>
                <h1>Real Article Headline</h1>
                <p>This is the real article text that the agent should read.</p>
            </body>
            </html>
        "#;

        let engine = HeadlessEngine::new();
        let resp = engine.process_page(html, "https://example.com/clean", 200, 80, None, false);

        assert!(resp.tui_render.contains("Real Article Headline"));
        assert!(resp.tui_render.contains("This is the real article text"));
        assert!(!resp.tui_render.contains("storing of cookies"));
        assert!(!resp.tui_render.contains("Accept All Cookies"));
    }
}
