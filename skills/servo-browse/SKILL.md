---
name: servo-browse
description: Headless agentic browser client for subshells with Lynx-grade terminal rendering and standard IAB ad perception.
---

# Servo-Browse Agent Skill

## Tool Definition
- **Command**: `servo-browse <url> [options]`
- **Flags**:
  - `--format <tui|json|png>`: Output presentation mode (default: `tui`).
  - `--cols <number>`: Viewport column width for text wrapping (default: `80`).
  - `--selector <css>`: CSS selector to scope extraction to a specific DOM subtree.
  - `--detect-ads <bool>`: Detect and extract standard IAB ad slots (default: `true`).
  - `--output <path>`: Destination path for full or element screenshot.

## Rendering & Presentation Contract
When receiving output from `servo-browse`:

1. **Verbatim TUI Presentation**:
   - Present `tui_render` inside a single fenced code block tagged `text` or preserve the Lynx-style layout verbatim.
   - Do NOT collapse Unicode box-drawing borders (`═`), whitespace indentation, or numbered footnotes `[n]`.
   - Do NOT rewrite or summarize the rendered text block unless the user explicitly requests a summary.

2. **IAB Ad Metadata**:
   - When ads are detected in `json` mode, present them in a clean summary table:
     | Slot ID | IAB Format | Dimensions | Destination URL |
     | :--- | :--- | :--- | :--- |
   - Never display raw ad tracking pixels, scripts, or base64 blobs to the user.

3. **Link Follow Actions**:
   - When the user selects a link by index (e.g., "open [2]" or "go to [3]"), run `servo-browse` with the matching reference URL from the references section.

4. **Multi-Agent Relay Guarantee**:
   - Subagents forwarding browser results to orchestrating agents must wrap `tui_render` in `<terminal_view>` blocks to prevent the orchestrator from altering terminal formatting.
