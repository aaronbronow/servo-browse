//! Headless browser engine coordinator and DOM extractor.

use std::path::Path;
use scraper::{Html, Selector};
use url::Url;
use crate::ads::detector::AdDetector;
use crate::contract::{AgentResponse, DetectedAd, IndexedLink, PageMetadata};
use crate::formatter::lynx::LynxFormatter;

pub struct HeadlessEngine {
    detector: AdDetector,
}

impl HeadlessEngine {
    pub fn new() -> Self {
        Self {
            detector: AdDetector::new(),
        }
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
            let ad_selector = Selector::parse(".ad-slot, [data-iab], [id*='leaderboard'], [id*='med-rect'], [id*='banner'], [id*='ad'], [class*='ad-']").unwrap();
            for element in document.select(&ad_selector) {
                let slot_id = element.value().attr("id").unwrap_or("ad-slot");

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
            if let Ok(re_slot) = regex::Regex::new(r#"defineSlot\s*\(\s*["'][^"']+["']\s*,\s*\[\s*(\d+)\s*,\s*(\d+)\s*\]\s*,\s*["']([^"']+)["']"#) {
                for script_el in document.select(&script_selector) {
                    let script_text = script_el.text().collect::<Vec<_>>().join(" ");
                    for cap in re_slot.captures_iter(&script_text) {
                        if let (Ok(w), Ok(h)) = (cap[1].parse::<u32>(), cap[2].parse::<u32>()) {
                            let slot_id = &cap[3];
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

        // 4. Format into Lynx TUI representation
        let formatter = LynxFormatter::new(cols);
        let tui_render = formatter.format(&title, &body_content, &indexed_links);

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
}
