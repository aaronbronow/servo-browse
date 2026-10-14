---
name: servo-browse
description: Headless agentic browser client for subshells with Lynx-grade terminal rendering, standard IAB ad perception, visual creative cropping, and bot challenge diagnostics.
---

# Servo-Browse Agent Skill

## Tool Definition
- **Command**: `servo-browse <url> [options]`
- **Flags**:
  - `--engine <servo|fast>`: Browser engine backend. `servo` (default) runs in-process SpiderMonkey JS, WebRender, and ad lifecycle interceptors. `fast` performs direct static HTML extraction without JS.
  - `--format <tui|json|png>`: Output presentation mode (default: `tui`).
  - `--cols <number>`: Viewport column width for text wrapping (default: `80`).
  - `--selector <css>`: CSS selector to scope extraction to a specific DOM subtree.
  - `--detect-ads <bool>`: Detect and extract standard IAB ad slots (default: `true`).
  - `--crop-ads`: Visually crops detected ad creatives to disk as PNG images.
  - `--ads-dir <path>`: Output directory for cropped ad creatives (default: `.agent/artifacts/ads`).
  - `--consent <opt-in|opt-out|ignore>`: Automated GDPR / CMP consent management (default: `opt-in`). Automatically handles OneTrust, IAB TCF v2.2, Google Funding Choices, and CCPA/US Privacy.
  - `--timeout <secs>`: Navigation and layout settlement timeout in seconds (default: `30`).
  - `--output <path>`: Destination path for full-viewport PNG screenshot.

## Rendering & Presentation Contract
When receiving output from `servo-browse`:

1. **GDPR / CMP Auto-Consent & Clean Presentation**:
   - `servo-browse` automatically resolves consent banners (OneTrust, Sourcepoint, Didomi, Google FC) and stubs IAB TCF `__tcfapi` to unblock ad auctions and dynamic publisher content.
   - All legal cookie disclaimers and CMP banner containers are automatically filtered out from Lynx TUI and markdown outputs to ensure high-signal text.

2. **Verbatim TUI Presentation**:
   - Present `tui_render` inside a single fenced code block tagged `text` or preserve the Lynx-style layout verbatim.
   - Do NOT collapse Unicode box-drawing borders (`═`), whitespace indentation, or numbered footnotes `[n]`.
   - Do NOT rewrite or summarize the rendered text block unless the user explicitly requests a summary.

2. **IAB Ad Metadata**:
   - When ads are detected in `json` mode, present them in a clean summary table:
     | Slot ID | IAB Format | Dimensions | Destination URL | Local Crop |
     | :--- | :--- | :--- | :--- | :--- |
   - When `--crop-ads` is enabled, `local_crop_path` contains the saved PNG path (e.g. `.agent/artifacts/ads/slot_name_300x250.png`).
   - Never display raw ad tracking pixels, scripts, or base64 blobs to the user.

3. **Bot Challenge & Mitigation Perception**:
   - When `bot_mitigation` is non-null (`challenge_detected: true`), a bot gate (Cloudflare Turnstile, DataDome, Akamai Bot Manager, AWS WAF, or CAPTCHA) blocked page delivery.
   - The TUI view will display a prominent top box warning banner.
   - **Crucial Agent Behavior**: Do NOT assume the site has no ads or treat challenge text as target content. Inspect `bot_mitigation.agent_advice` and `bot_mitigation.provider` to inform the user or adjust the browsing strategy.

4. **Link Follow Actions**:
   - When the user selects a link by index (e.g., "open [2]" or "go to [3]"), run `servo-browse` with the matching reference URL from the references section.

5. **Multi-Agent Relay Guarantee**:
   - Subagents forwarding browser results to orchestrating agents must wrap `tui_render` in `<terminal_view>` blocks to prevent the orchestrator from altering terminal formatting.
   - `stdout` is strictly JSON or verbatim TUI; all engine diagnostics route to `stderr`.
