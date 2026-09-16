# Project Memory: servo-browse

Headless agentic browser engine based on upstream Servo (`servo = "0.5.0"`), optimized for subshells (Bash, MSYS2) and agent harnesses (Antigravity CLI, Buzz, Goose).

---

## 1. Core Architectural Principles

1. **Lynx-Grade Terminal Formatting**:
   - Numbered inline footnote links (`[1]`, `[2]`, `[3]`) backed by a pinned references index table at the bottom.
   - Word wrapping to terminal column widths (`--cols 80`).
   - Unicode box-drawing borders for headers and structured semantic blocks.
2. **First-Class IAB Display Ad Perception**:
   - Intercepts ad lifecycle event buses (Google Publisher Tag `slotRenderEnded` / Prebid).
   - Validates computed layout boxes against standard IAB dimensional profiles (Leaderboard 728×90, Medium Rectangle 300×250, Large Rectangle 336×280, Half-Page 300×600, Billboard 970×250, Mobile Banner 320×50).
   - Detects both in-DOM ad containers and in-page `googletag.defineSlot` script declarations.
   - Emits structured ad metadata (placement, dimensions, destination URL) in JSON mode while keeping body text clean.
3. **Subshell & Stream Safety**:
   - All operational logs, diagnostics, and SpiderMonkey traces route strictly to `stderr`.
   - `stdout` is strictly reserved for machine-readable JSON or verbatim TUI views.
   - Handles `BrokenPipe` (SIGPIPE) gracefully on `stdout` when piped to Unix tools (e.g. `head`, `less`).
4. **Headless Offscreen Rendering (No Virtual Display)**:
   - TUI and JSON modes compute layout purely in memory without graphics surfaces.
   - PNG/graphic mode uses software offscreen EGL (`EGL_MESA_platform_surfaceless` / Mesa `llvmpipe`) rather than relying on `xvfb-run`.
5. **Direct Upstream Dependency**:
   - Targets the official `servo` crate directly (`servo = "0.5.0"`), bypassing scraping wrappers like `servo-fetch` that strip ad slots.

---

## 2. Verified Live Testbeds

### Google Publisher Tag (GPT) Sample Pages (No Bot Blocks)
- **Display Test Ad (300×250 Medium Rectangle)**:
  ```bash
  cargo run --quiet -- "https://googleads.github.io/google-publisher-tag-samples/display-test-ad/js/demo.html" --format tui
  ```
- **Ad Sizes (Multi-unit Responsive)**:
  ```bash
  cargo run --quiet -- "https://googleads.github.io/google-publisher-tag-samples/ad-sizes/js/demo.html" --format json
  ```

### Local Fixtures
- `fixtures/iab_sample.html`: Contains static 728×90 Leaderboard and 300×250 Medium Rectangle with anchor links.

### Commercial Anti-Bot Notes
- Commercial properties often run edge bot challenges before serving content or ad tags:
  - **Expedia**: Uses Akamai Bot Manager (`root-challenge` / `captcha-pwa`), returns `HTTP 429` with challenge scripts.
  - **Allrecipes**: Uses Cloudflare Turnstile (`cf-mitigated: challenge`), returns `HTTP 403`.
- Inspecting live commercial ads requires the full SpiderMonkey JS engine and browser environment emulation (Phase 4 & 5) to solve challenges and execute in-page ad auctions.

---

## 3. Development Commands

```bash
# Set environment
source "$HOME/.cargo/env"

# Run test suite
cargo test

# Run TUI extraction
cargo run --quiet -- "https://example.com" --format tui

# Run JSON extraction
cargo run --quiet -- "https://example.com" --format json | jq .

# Scope extraction to specific DOM subtree
cargo run --quiet -- "https://en.wikipedia.org/wiki/Lynx_(web_browser)" --format tui --selector "main"
```
