use crate::contract::IndexedLink;
use textwrap::wrap;

/// Formats text content and links into a Lynx-style indexed terminal layout.
pub struct LynxFormatter {
    pub width: usize,
}

impl LynxFormatter {
    pub fn new(width: usize) -> Self {
        Self {
            width: width.max(40),
        }
    }

    /// Formats a page into a Lynx-style view:
    /// 1. Top box-drawing banner for the title.
    /// 2. Word-wrapped text matching the configured width.
    /// 3. Pinned references section mapping indices to URLs.
    pub fn format(&self, title: &str, body: &str, links: &[IndexedLink]) -> String {
        let mut out = String::new();
        let border_len = self.width.min(80);
        let border = "═".repeat(border_len);

        // Header frame
        out.push_str(&border);
        out.push('\n');
        out.push_str("  ");
        out.push_str(title);
        out.push('\n');
        out.push_str(&border);
        out.push_str("\n\n");

        // Body with hard line wrapping
        for line in body.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                out.push('\n');
            } else {
                for wrapped in wrap(line, self.width) {
                    out.push_str(&wrapped);
                    out.push('\n');
                }
            }
        }

        // Lynx-style References section
        if !links.is_empty() {
            let sep_len = border_len.saturating_sub(16);
            let sep = "─".repeat(sep_len);
            out.push_str(&format!("\n── References {} \n", sep));
            for link in links {
                out.push_str(&format!("[{}] {}\n", link.index, link.url));
            }
        }

        out
    }
}

impl Default for LynxFormatter {
    fn default() -> Self {
        Self::new(80)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lynx_formatter_output() {
        let formatter = LynxFormatter::new(60);
        let title = "Test Article Title";
        let body = "This is a paragraph with an inline link [1] that describes the topic.";
        let links = vec![IndexedLink {
            index: 1,
            text: "inline link".to_string(),
            url: "https://example.com/topic".to_string(),
        }];

        let result = formatter.format(title, body, &links);
        assert!(result.contains("═══"));
        assert!(result.contains("Test Article Title"));
        assert!(result.contains("[1]"));
        assert!(result.contains("── References"));
        assert!(result.contains("[1] https://example.com/topic"));
    }

    #[test]
    fn test_empty_links() {
        let formatter = LynxFormatter::new(80);
        let result = formatter.format("No Links", "Just plain text.", &[]);
        assert!(!result.contains("── References"));
    }
}
