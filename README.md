# servo-browse

An agentic headless browser engine built on Servo, designed for execution inside subshells (Bash, MSYS2) by AI agent harnesses (Buzz, Goose, Antigravity).

## Key Features
- **Lynx-Grade Terminal Rendering**: Emits line-wrapped terminal views with numbered footnote references (`[1]`, `[2]`) and box-drawing borders.
- **IAB Ad Perception**: Detects standard IAB ad units (Leaderboard, Medium Rectangle, etc.), extracting destination URLs and bounding boxes without stripping them via ad-blockers.
- **Subshell & Stream Isolation**: Emits clean JSON/TUI to `stdout` while routing engine logs and diagnostics strictly to `stderr`.
- **Headless Software Offscreen Graphics**: Designed to render without external X11 display servers (`xvfb-run`) using software EGL/llvmpipe.

## CLI Usage
```bash
# Render a page in Lynx-style formatted text
servo-browse "https://example.com" --format tui

# Output structured JSON with separate content, links, and detected ads
servo-browse "https://example.com" --format json

# Scope rendering to a specific CSS selector
servo-browse "https://example.com" --selector "article.main"

# Specify terminal width (default: 80)
servo-browse "https://example.com" --cols 100
```
