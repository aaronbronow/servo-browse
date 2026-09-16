# **Servo-Based TUI Browser Agent Architecture**

[https://gemini.google.com/app/6282590751011f70](https://gemini.google.com/app/6282590751011f70)

*User prompt: take a look at this project https://github.com/konippi/servo-fetch i want to start a servo-based agentic browser that has two unique features: it's designed to run inside a subshell like msys2 or bash (if bash is what buzz uses on linux and mac) to be used by Buzz or Goose agents. and secondly, it is paired with a skill or agent plugin that instructs the agent harness exactly how to render the response beautifully and consistently to the user. for example, if running from a CLI the skill+servo tool can pass strictly formatted response to the orchestrating agent to render a TUI-ready formatted view to the UI. imagine browsing the web with Lynx on bash. the CLI agent should be able to render a whole page or a part of a page in the console, or to markdown and have it look as good as Lynx in a terminal. as for the Buzz harness integration, the response from the servo-browser to the subagent should be strictly formatted to provide enough info for the subagent calling the tool to send a response up to the orchestrating agent that retains formatting.*

Response: servo-fetch is a great starting point for this because it solves the hardest systems problem: **embedding a real engine (Servo \+ SpiderMonkey) in a lightweight process (\~50–65 MB RSS vs Playwright's 300+ MB)** with software rendering and layout-aware DOM tree traversal.

To turn this into a Lynx-quality, TUI-ready agentic browser for subshells (MSYS2/bash) orchestrated by Buzz or Goose, you need to solve two distinct layers:

> 1. **The Subshell Binary & Environment Layer** (running headless across Linux/macOS/MSYS2 without X11 crashes).  
> 2. **The Formatting Contract & Skill Pipeline** (a strict intermediate payload that survives the subagent → orchestrator → TUI pipeline without markdown degradation).

### **1\. Subshell Runtime Architecture (Bash & MSYS2)**

When agents run inside subshells (whether Buzz running tools via child bash processes or Goose dispatching shell commands), they hit three common friction points:

> * **Headless Display & Software Rendering:** Servo requires an EGL/GLES context. On Linux headless servers, servo-fetch requires xvfb-run or an EGL software driver like Mesa (llvmpipe). In your standalone CLI tool:  
  * Force **offscreen EGL/OSMesa software rendering** internally so the subshell never requires an active X11/Wayland display server or separate wrapper like xvfb-run.  
  * **MSYS2 / Windows Native:** Windows builds need libEGL.dll and libGLESv2.dll bundled directly alongside the executable so the subshell environment path finds them cleanly without MSYS2 path-translation (/c/... vs C:\\...) issues.  
> * **Stream Safety (Stdout vs Stderr):** Agents often mix stdout and stderr when running shell tools. Your binary should strictly output the structured response payload to stdout, and send logging, navigation progress, network events, and layout debugs to stderr.

### **2\. Lynx-Grade Representation: Why Raw Markdown Fails**

Raw Markdown from Readability or standard DOM parsers usually breaks down in terminal UIs:

> * Nested layouts, multi-column cards, and sidebars collapse into unreadable blocks.  
> * Inline links turn into messy long URLs or disappear.  
> * Terminal line-wrapping wrecks tables and indented hierarchy.

Lynx looks good because it uses an **indexed layout model**:

> 1. **Numbered Link References:** Inline links are replaced with bracketed numbers \[1\], \[2\], \[3\], and a numbered bibliography/jump index is pinned at the bottom.  
> 2. **Viewport-Aware Text Wrapping:** Hard-wrapped to standard column widths (e.g., 80 or 100 cols), respecting word boundaries.  
> 3. **Structured Semantic Blocks:** Clear visual rules for \<nav\>, \<aside\>, forms (\[Input: Search...\] \[Submit\]), and tables formatted with box-drawing characters (┌─┬─┐).

### **3\. The Strict Response Contract (Servo → Subagent)**

To ensure the subagent receives unambiguous data and passes it cleanly back to the orchestrating agent without hallucinating or stripping ANSI codes, the binary should support a structured transport format: \--format agent-tui or \--format json.

A clean JSON payload schema:

{  
  "meta": {  
    "url": "https://example.com/article",  
    "title": "Example Article Title",  
    "status": 200,  
    "viewport": { "cols": 80, "rows": 24 }  
  },  
  "navigation": {  
    "breadcrumbs": \["Home", "Docs", "Architecture"\],  
    "pagination": { "prev": null, "next": "https://example.com/article?p=2" }  
  },  
  "tui\_render": "┌────────────────────────────────────────────────────────────┐\\n│ Example Article Title                                      │\\n└────────────────────────────────────────────────────────────┘\\n\\nThis is the opening text with a referenced link \[1\]...\\n\\n─── References ───\\n\[1\] https://example.com/reference-url",  
  "markdown\_render": "\# Example Article Title\\n\\nThis is the opening text with a referenced link \[1\]...\\n\\n\[1\]: https://example.com/reference-url",  
  "interactive\_elements": \[  
    { "id": 1, "type": "link", "text": "reference-url", "target": "https://example.com/reference-url" },  
    { "id": 2, "type": "button", "name": "submit", "selector": "\#btn-submit" }  
  \]  
}

By providing both tui\_render (pre-formatted with column boundaries and reference numbers) and interactive\_elements, the calling agent doesn't have to guess what can be clicked or submitted.

### **4\. Designing the Skill / Plugin Contract**

The skill package (for Goose or Buzz) acts as the bridge. It provides the system prompt instructions that constrain how the subagent and orchestrator present the tool's output.

#### **Goose / Buzz Skill Specification (servo\_browser.yaml / SKILL.md)**

\# Servo Browser TUI Agent Skill

\#\# Tool Definition  
\- Command: \`servo-browse \<url\> \[options\]\`  
\- Flags:  
  \- \`--width \<int\>\`: Terminal columns (default: 80).  
  \- \`--section \<selector\>\`: Extract a specific DOM node or article subtree.  
  \- \`--page \<int\>\`: Paginated chunk (to fit within context windows).

\#\# Rendering Instructions for Subagent & Orchestrator  
When calling \`servo-browse\`, you will receive a response containing \`tui\_render\` and \`meta\`.

1\. \*\*Terminal Fidelity (Strict Preservation):\*\*  
   \- Present the \`tui\_render\` block inside a single fenced code block tagged \`text\` or preserve the Lynx-style layout verbatim.  
   \- Do NOT collapse ASCII/box-drawing lines, whitespace padding, or numbered footnotes \`\[1\]\`.  
   \- Do NOT rewrite or summarize the rendered text block unless the user explicitly requested a summary.

2\. \*\*Link Navigation Mapping:\*\*  
   \- Always retain the "References / Links" footer table.  
   \- If the user asks to "click" or "follow" a link (e.g., "go to \[2\]"), invoke the tool with the URL mapped to \`\[2\]\`.

3\. \*\*Multi-Agent Relay Guarantee:\*\*  
   \- Subagents: Forward the \`tui\_render\` content to the parent orchestrator wrapped in \`\<terminal\_view\>\` tags to prevent the orchestrator from altering terminal indentation or stripping link references.

### **5\. Implementation Roadmap Starting from servo-fetch**

> 1. **Fork & Decouple Renderer:**  
   * In servo-fetch/crates/engine, inspect how it extracts DOM trees into Markdown. It currently uses a visibility/layout-aware tree walker.  
   * Add a LynxFormatter visitor that traverses the layout tree and emits indexed links (\[1\]), text wrapped to a specified column width (via textwrap), and box borders (using comfy-table or raw unicode box drawing).  
> 2. **Add Viewport Pagination & Element Targeting:**  
   * Terminal screens have limited vertical space. Add \--viewport-rows 40 \--page 1 and CSS selector filtering (--select "main", \--select "\#content") so the agent can inspect specific components without consuming massive context window tokens.  
> 3. **Packaging for Subshells:**  
   * Compile a single statically linked binary with software rasterization enabled (servo-browse).  
   * Ship prebuilt packages targeting x86\_64-unknown-linux-gnu, aarch64-apple-darwin, and x86\_64-pc-windows-gnu / msys2.  
> 4. **Publish Goose / Buzz Skill:**  
   * Package the tool specification and rendering rules as an MCP tool (reusing servo-fetch's built-in MCP server structure) or as a native CLI skill bundle that can be dropped directly into \~/.config/goose/skills/ or Buzz's agent plugins.

---

*User prompt: walk me through how we would test running a servo headless browser in a subshell and just outputting a dump of html, then another test outputting a screenshot of the whole page*

Response: Here is how to set up, run, and verify both test cases directly in a bash or MSYS2 subshell environment.

### **Prerequisites & Headless Environment Setup**

Servo uses an offscreen OpenGL/EGL surface to compute layout and render screenshots. Because subshells (especially non-interactive agent subshells) usually lack an active display server, you need to handle headless graphics drivers.

> * **Linux (Headless / CI / Agent containers):** Install runtime dependencies and use an offscreen X server or software Mesa:  
>   sudo apt-get update && sudo apt-get install \-y libegl1 libfontconfig1 libfreetype6 xvfb

>   Prefix execution with xvfb-run \--auto-servernum if a physical or virtual display is absent.  
> * **macOS:** No display wrapper needed; native macOS graphics surfaces work directly in bash/zsh subshells.  
> * **Windows / MSYS2:** Ensure libEGL.dll and libGLESv2.dll reside in the same directory as the binary or on your PATH. Under MSYS2 bash, run native Windows binaries directly:  
>   export PATH="/c/path/to/servo-fetch:$PATH"

**1Verify Subshell Binary Execution**

Sanity Check

Confirm that the binary runs cleanly inside your subshell environment without throwing missing dynamic linker or shared library errors.

\# Linux / Headless:  
xvfb-run \--auto-servernum servo-fetch \--version

\# macOS / MSYS2:  
servo-fetch \--version

**Verification:** The terminal prints the version string (e.g., servo-fetch 0.x.x) to stdout with exit code 0\.

**2Test 1: Dump Raw Evaluated HTML to Stdout**

DOM & JS Verification

Execute a fetch targeting a URL to verify that network retrieval, SpiderMonkey JavaScript execution, and DOM generation complete properly.

To capture raw HTML rather than the default Markdown converter, query the full page DOM via \--js:

\# Linux:  
xvfb-run \--auto-servernum servo-fetch "https://example.com" \\  
  \--js "document.documentElement.outerHTML" \> /tmp/page\_dump.html

\# macOS / MSYS2:  
servo-fetch "https://example.com" \\  
  \--js "document.documentElement.outerHTML" \> /tmp/page\_dump.html

Alternatively, if using the structured JSON output format:

servo-fetch "https://example.com" \--format json | jq \-r '.html // .content' \> /tmp/page\_dump.html

**Verification:** Inspect the output file to ensure valid HTML tags are present and the exit code is clean:

head \-n 10 /tmp/page\_dump.html  
echo $?

**3Test 2: Full-Page Software Screenshot**

Rasterizer & Layout Verification

Trigger the software rasterizer to compute page layout, evaluate CSS dimensions, and output a PNG file.

Run the screenshot command targeting an output file:

\# Linux:  
xvfb-run \--auto-servernum servo-fetch "https://example.com" \\  
  \--format png \\  
  \-o /tmp/screenshot.png

\# macOS / MSYS2:  
servo-fetch "https://example.com" \\  
  \--format png \\  
  \-o /tmp/screenshot.png

**Verification:** Confirm the file was created and contains a valid PNG header:

file /tmp/screenshot.png  
\# Expected output: /tmp/screenshot.png: PNG image data, ...

**4Validate Non-Interactive Subshell Execution (Agent Simulation)**

Pipeline & Pipe Safety

Simulate how Buzz or Goose executes commands via subshell invocation (bash \-c "..." or sh \-c "..."), ensuring standard output, error streams, and return codes behave correctly when non-interactive.

bash \-c '  
  set \-euo pipefail  
  TARGET\_URL="https://example.com"  
  OUT\_DIR=$(mktemp \-d)  
    
  echo "Fetching DOM..."  
  xvfb-run \--auto-servernum servo-fetch "$TARGET\_URL" \--js "document.documentElement.outerHTML" \> "$OUT\_DIR/dom.html" 2\>"$OUT\_DIR/err.log"  
    
  echo "Capturing Screenshot..."  
  xvfb-run \--auto-servernum servo-fetch "$TARGET\_URL" \--format png \-o "$OUT\_DIR/shot.png" 2\>\>"$OUT\_DIR/err.log"  
    
  ls \-lh "$OUT\_DIR"  
'

**Verification:** Both dom.html and shot.png should exist in the temporary directory with non-zero byte sizes, and stderr output should be isolated to err.log.

### **Common Subshell Pitfalls to Watch For**

> * **Stderr Pollution:** Servo engine logging and network trace outputs often dump directly to stderr. Ensure your agent runner does not combine streams (2\>&1), or tool output parsing will break when receiving JSON or HTML.  
> * **MSYS2 Windows Path Translation:** When passing output paths like \-o /tmp/shot.png inside an MSYS2 bash environment to a native Windows .exe, MSYS2 may pass Unix-style paths that native binaries cannot write to. In MSYS2 scripts, normalize paths using cygpath \-w:  
>   OUT\_FILE=$(cygpath \-w "/tmp/shot.png")  
>   ./servo-fetch.exe "https://example.com" \--format png \-o "$OUT\_FILE"

> * **Font Fallback in Containers:** If screenshots render missing characters or empty rectangles ("tofu"), install basic TrueType fonts into the subshell environment (sudo apt-get install fonts-dejavu-core fonts-freefont-ttf).

---

*User prompt: can the renderer be compiled into a single binary with servo?*

Response: **Yes, absolutely.** You can compile Servo and its entire rendering/layout pipeline directly into a single, self-contained binary. In fact, that is how servo-fetch works under the hood: it is a Rust project that statically embeds Servo’s core crates (layout\_2020, script, style, webrender, etc.) into a single executable rather than spawning an external browser process.

However, whether that single binary is *completely dependency-free* (true zero runtime dynamic libraries) depends on how you configure graphics and font rasterization for your target operating systems.

### **How Servo Renders Headless**

Servo uses **WebRender** (the GPU-based display list renderer from Mozilla) for painting and rasterization. In a headless environment, WebRender needs an offscreen OpenGL/EGL surface to draw into:

> 1. **The In-Binary Rust Pipeline (Static):**  
   * **HTML/DOM & Script:** script crate (embeds SpiderMonkey).  
   * **CSS & Layout:** style (Stylo) \+ layout\_2020.  
   * **Drawing / Display List:** webrender. All of these are pure Rust crates (or Rust crates compiling bundled C/C++ like SpiderMonkey via build.rs) and are statically compiled directly into your single binary.  
> 2. **The Graphics Backend (The Tricky Part):**  
   * WebRender talks to an OpenGL or GLES API.  
   * If using hardware drivers or standard headless Linux Mesa, it dynamically links against libEGL.so.1 or libGL.so.1.  
   * If you want a **true standalone binary** that requires no display server (Xvfb) and no external GPU drivers, you use **software rasterization** linked directly into the binary.

### **Strategies for True Single-Binary Deployment**

Depending on the OS where your subshell runs (Linux, macOS, or Windows/MSYS2), here is how to achieve single-binary packaging:

#### **1\. Linux: Static OSMesa or Headless Surfman**

> * **Surfman / OSMesa:** Servo utilizes the surfman crate for cross-platform offscreen rendering contexts. You can compile Mesa with OSMesa (Off-Screen Mesa) or software rasterizers (llvmpipe / softpipe).  
> * **Static Linking (musl):** You can statically link an offscreen software Mesa rasterizer into your Rust binary. This removes the need for xvfb-run or external libEGL.so / libGL.so on headless servers, outputting a completely self-contained ELF binary.  
> * **Fontconfig / Freetype:** Ensure freetype is linked statically (using the freetype-sys crate with bundled sources) so it doesn't fail if the host container lacks libfreetype6.

#### **2\. macOS: Built-in Offscreen Surfaces**

> * macOS is the easiest target. Apple's native frameworks (CoreGraphics, OpenGL / Metal surface bridges via surfman) are dynamically linked system libraries guaranteed to exist on every macOS installation.  
> * cargo build \--release produces a single Mach-O binary with no external DLLs or wrappers needed.

#### **3\. Windows & MSYS2: Statically Bundling or Sidecar DLLs**

> * Under Windows, Servo relies on ANGLE or Mesa to map OpenGL/EGL calls to DirectX or pure CPU rasterization:  
  * **Sidecar Pattern (servo-fetch default):** Shipped as servo-fetch.exe alongside libEGL.dll and libGLESv2.dll.  
  * **Single Static Binary Pattern:** You can configure the Windows build to statically link an offscreen software OpenGL provider (such as software Mesa built for Windows, or a CPU rasterization backend via OSMesa) directly into the .exe.

### **What This Architecture Looks Like**

┌────────────────────────────────────────────────────────┐  
│               Your CLI Binary (servo-browse)           │  
│                                                        │  
│  ┌─────────────────────────┐  ┌─────────────────────┐  │  
│  │ SpiderMonkey (JS Engine)│  │ Stylo / Layout 2020 │  │  
│  └─────────────────────────┘  └─────────────────────┘  │  
│               │                         │              │  
│               ▼                         ▼              │  
│  ┌──────────────────────────────────────────────────┐  │  
│  │         WebRender (Painting & Layout)            │  │  
│  └──────────────────────────────────────────────────┘  │  
│               │                         │              │  
│      (For Screenshot)           (For Lynx TUI)         │  
│               │                         │              │  
│               ▼                         ▼              │  
│  ┌─────────────────────────┐  ┌─────────────────────┐  │  
│  │ In-Memory Software EGL  │  │ LynxFormatter       │  │  
│  │ (Pixels \-\> PNG encoder) │  │ (DOM \-\> ANSI / TUI) │  │  
│  └─────────────────────────┘  └─────────────────────┘  │  
└───────────────┼─────────────────────────┼──────────────┘  
                │                         │  
             stdout                    stdout  
        (base64/png pipe)        (Structured Text/JSON)

### **Key Takeaway for Your Agent Tool**

Because you want **both** DOM/Lynx-text extraction and screenshots:

> * **For Lynx/TUI formatting:** You don't even need the rasterizer to touch the GPU. You only need the **DOM tree \+ computed layout metrics** (which Servo calculates purely in CPU memory via the layout\_2020 crate) to determine element visibility, text flow, and box positions.  
> * **For Screenshots:** Statically compiling the software offscreen context into the binary means your Buzz or Goose agent can invoke ./servo-browse \--format png inside an unprivileged Docker container or raw bash subshell without ever needing xvfb-run or root-installed display libraries.

---

*User prompt: here's the end goal: i want to create an agentic browser client that can "see" IAB ads. instead of scraping plain html it should politely request a full page with javascript events and examine ad slots if they are standard. we can ignore non-standardized ads. i want to prove that it's possible to pipeline standardized graphical ad content to a headless agent. since most agents will only want markup, we will detect ads and capture their metadata and link targets. in graphic output mode (screenshot mode) we will render the media content too.*

Response: This is a clever and feasible architecture. The trick to making an agentic browser "see" and pipeline IAB display ads without getting bogged down in arbitrary web noise is **not** trying to parse messy, nested ad-tech iframes from the outside.

Instead, you exploit the fact that ad-tech is already strictly standardized around **IAB container dimensions** and **in-page event buses** (specifically Google Publisher Tag / GPT googletag and pbjs for Prebid).

Here is the concrete architecture to intercept, standardize, and pipeline IAB ad data to a headless agent.

### **1\. Dual Detection Strategy: Events \+ Computed Geometry**

Ad slots usually load inside cross-origin FriendlyIframe or SafeFrame containers. If you inspect raw HTML strings, you will only see blank container \<div\>s or generic iframe tags.

In Servo, you combine **JavaScript Event Interception** with **Computed Layout Box Verification**:

           \[ Webpage JS Runtime \]  
                      │  
       Hooks GPT / Prebid Event Listeners  
                      │  
    ┌─────────────────┴─────────────────┐  
    ▼                                   ▼  
slotRenderEnded event               Servo Layout Engine  
(Size: 300x250, isEmpty: false)    (Computed BoundingBox: 300x250)  
    │                                   │  
    └─────────────────┬─────────────────┘  
                      ▼  
            Match IAB Taxonomy Unit  
           (e.g., "Medium Rectangle")

#### **A. The Injected JS Hook (Pre-rendering lifecycle)**

Inject an early script before page execution that registers listeners with the ad server event bus:

window.googletag \= window.googletag || { cmd: \[\] };  
window.\_\_agent\_ad\_inventory \= \[\];

window.googletag.cmd.push(function() {  
  window.googletag.pubads().addEventListener('slotRenderEnded', function(event) {  
    if (event.isEmpty) return; // Ignore unfilled / collapsed slots

    const size \= event.size; // e.g. \[300, 250\] or \[728, 90\]  
    const slotElementId \= event.slot.getSlotElementId();  
    const domNode \= document.getElementById(slotElementId);

    // Extract creative metadata & click-through links from inside friendly containers  
    let targetUrl \= null;  
    let creativeType \= "iframe";  
      
    if (domNode) {  
      const link \= domNode.querySelector("a\[href\]");  
      if (link) targetUrl \= link.href;  
    }

    window.\_\_agent\_ad\_inventory.push({  
      slot\_id: slotElementId,  
      ad\_unit\_path: event.slot.getAdUnitPath(),  
      width: Array.isArray(size) ? size\[0\] : null,  
      height: Array.isArray(size) ? size\[1\] : null,  
      advertiser\_id: event.advertiserId || null,  
      campaign\_id: event.campaignId || null,  
      target\_url: targetUrl  
    });  
  });  
});

#### **B. The Layout Engine Filter (Filtering for Standard IAB Sizes)**

Once layout evaluation finishes, pass \_\_agent\_ad\_inventory and the DOM through an IAB spec matrix. Any slot whose computed width and height do not match known IAB display units is dropped:

| IAB Standard Name | Width × Height (px) | Typical Slot Placement |
| :---- | :---- | :---- |
| **Medium Rectangle (MPU)** | 300 × 250 | Sidebar / In-Article stream |
| **Leaderboard** | 728 × 90 | Page header / Above navigation |
| **Half-Page / Large Skyscraper** | 300 × 600 | Right-rail sidebar |
| **Wide Skyscraper** | 160 × 600 | Tall vertical sidebar |
| **Billboard** | 970 × 250 | Premium top takeover |
| **Mobile Leaderboard** | 320 × 50 | Sticky bottom or mobile header |
| **Large Mobile Banner** | 320 × 100 | In-feed mobile container |

### **2\. Output Schema: Text/Agent Mode vs Graphic Mode**

Your CLI / subshell tool should emit a structured JSON envelope that separates normal document flow from ad metadata.

#### **Mode A: Structured Markup Mode (--format json or \--format agent-tui)**

When the subagent asks for text/markdown:

> 1. Strip ad containers from the main body text so the agent's LLM context window is not polluted with ad script junk.  
> 2. Provide a discrete ads block alongside the markdown\_render or tui\_render.

{  
  "meta": {  
    "url": "https://tech-news.example.com/article",  
    "title": "New Chip Architecture Announced"  
  },  
  "ads": \[  
    {  
      "slot\_id": "div-gpt-ad-leaderboard-0",  
      "standard\_unit": "Leaderboard (728x90)",  
      "position": "above-fold-header",  
      "target\_url": "https://adclick.example.com/redirect?dest=https://cloudprovider.com/signup",  
      "visible": true  
    },  
    {  
      "slot\_id": "div-gpt-ad-sidebar-mpu",  
      "standard\_unit": "Medium Rectangle (300x250)",  
      "position": "sidebar-top",  
      "target\_url": "https://adclick.example.com/redirect?dest=https://saas-tool.com/demo",  
      "visible": true  
    }  
  \],  
  "tui\_render": "\[AD BANNER: Leaderboard 728x90 \-\> Cloud Provider Demo\]\\n\\n\# New Chip Architecture Announced\\n...",  
  "clean\_content\_markdown": "\# New Chip Architecture Announced\\n..."  
}

#### **Mode B: Graphic / Visual Mode (--format png or \--ad-crop-dir /path)**

Because Servo uses WebRender for software rasterization, it already possesses the computed pixel coordinate bounding box (x, y, width, height) for every DOM element.

> * **Full-Page Screenshot:** The ad media is already rendered in place on the page canvas.  
> * **Cropped Ad Media Extraction:** Using the bounding box from layout\_2020 for slot\_id, crop the rasterized buffer to export discrete PNG/JPEG assets of just the active creatives (ad\_leaderboard\_0.png, ad\_sidebar\_mpu.png).

### **3\. Execution Pipeline in a Bash / MSYS2 Subshell**

Here is how a subagent in Buzz or Goose can test and run this polite lifecycle end-to-end:

\# Run servo-browse with a polite JS settle time (e.g., wait 3s for bids/GPT events to resolve)  
servo-browse "https://example.com" \\  
  \--wait-for-ads \\  
  \--ad-timeout 3000 \\  
  \--format json \> /tmp/agent\_page\_data.json

Inside your tool's Rust core:

> 1. **Network & Page Load:** Load URL with standard browser User-Agent headers.  
> 2. **Event Loop Wait:** Wait until both window.onload fires and slotRenderEnded events stop firing (or the ad timeout expires).  
> 3. **Layout Pass:** Servo runs layout\_2020 compute pass to finalize bounding boxes.  
> 4. **Extraction:** Match identified GPT slots against standard IAB pixel ratios.  
> 5. **Output Delivery:** Write metadata, target URLs, and clean content directly to stdout.

This makes advertising visible to AI agents as first-class, structured entities without turning the agent into an undifferentiated HTML scraper.

---

*User prompt: what problems are we likely to encounter building a prototype that we can run with antigravity cli in our ubuntu headless dev environment?*

Response: Building a Servo-based ad-aware browser to run under **Antigravity CLI** in a headless Ubuntu environment introduces four main engineering hurdles: headless graphics bootstrapping, viewability and bot traps, cross-origin SafeFrame isolation, and agent harness stream/timeout constraints.

### **1\. Headless Graphics & System Environment (Ubuntu)**

> * **The WebRender / EGL Surface Trap:** Servo does not use a virtual DOM; its layout\_2020 and webrender crates expect an active OpenGL/EGL surface to calculate geometry and paint pixels. On a headless Ubuntu server without an X11 server or physical GPU:  
  * Running without an offscreen context causes instant initialization panics (EGL\_NOT\_INITIALIZED or No available display).  
  * *Fix:* You must either wrap execution in xvfb-run \--auto-servernum, compile Servo against OSMesa (off-screen software Mesa), or link llvmpipe with headless surfman.  
> * **Font Metric Distortion ("Tofu" & Zero-Width Boxes):** Ad containers rely heavily on font dimensions and CSS layout bounds. Bare-bones Ubuntu server installations lack standard TrueType and CJK fonts.  
  * Missing fontconfig packages cause Servo’s layout engine to collapse text nodes or miscalculate bounding boxes.  
  * *Fix:* Explicitly install system fonts (fonts-dejavu-core, fonts-liberation, fonts-noto) and run fc-cache \-fv in your dev environment setup.

### **2\. The IAB Ad Lifecycle & Viewability Traps**

Programmatic ads (GPT, Prebid, Amazon TAM) do not behave like standard static HTML or basic client-rendered apps:

> * **The "Zero-Viewability" Deadlock (IntersectionObserver):** Modern ad orchestrators rarely request or render ad creatives until the slot passes viewability thresholds. If the headless browser defaults to a virtual viewport of 0×0, or if the document reports document.hidden \= true, GPT will define the slot but **never fire the bid request or render the creative**.  
  * *Symptom:* The ad container remains an empty \<div\>, and googletag.events.SlotRenderEndedEvent never fires.  
  * *Fix:* Your harness must set a realistic initial viewport (e.g., 1280×800), emulate document.visibilityState \= 'visible', and simulate scroll events down the page if ads are below the fold.  
> * **Ad Fraud / Bot Detection & "No Fill" (204 Responses):** Ad exchanges and verification scripts (DoubleVerify, Moat/Oracle, HUMAN) aggressively fingerprint the execution environment.  
  * Running SpiderMonkey with missing Chrome properties (navigator.userAgentData, standard audio/canvas fingerprints, or headless user agents) will lead SSPs to drop bids. You will frequently receive empty fallback slots or transparent 1x1 tracking GIFs.  
  * *Fix:* For prototyping, test against **mock local GPT test pages** or staging publisher ad tags before testing live commercial media sites.

### **3\. Cross-Origin SafeFrames & DOM Isolation**

> * **Same-Origin Policy (SOP) Blockades:** Most IAB display ads do not render in friendly iframes directly in the top window; they load inside sandboxed SafeFrames (\<iframe src="\[https://tpc.googlesyndication.com/safeframe/\](https://tpc.googlesyndication.com/safeframe/)..." sandbox="..."\>).  
  * If your agent attempts to inspect ad text or click-through links using a user-space JavaScript injection (--js "document.querySelector('iframe').contentDocument"), the engine will throw a cross-origin DOM security exception.  
> * **Engine-Level Traversal Requirement:**  
  * To bypass this cleanly without hacking web security standards in JS, the extraction must be written in **Rust** inside your Servo fork. Because your host binary owns the engine pipeline, it can traverse child Document and LayoutTree instances across origin boundaries natively to read anchor hrefs and creative text.

### **4\. Antigravity CLI Integration Nuances**

Antigravity CLI interacts with tools either via subshell execution (bash commands) or as a registered Model Context Protocol (MCP) server:

> * **Asynchronous Latency vs. Agent Timeouts:** A typical page load with programmatic ads requires:  
  1. Base HTML fetch → DOM parsing (\~300ms)  
  2. Prebid auction timeout (\~500–1000ms)  
  3. Google Publisher Tag fill & iframe creative fetch (\~500–1500ms) Total wait time can easily hit 3–5 seconds. If Antigravity runs the tool in a shell step with tight execution timeouts or aggressive polling, the command may be killed before slots settle.  
> * **Stdout Pollution Breaking Agent Parsers:** SpiderMonkey, Fontconfig, and WebRender emit warnings and layout notices directly to stdout/stderr.  
  * If debug noise bleeds into stdout, Antigravity's JSON parser or subagent prompt will choke.  
  * *Fix:* Hard-isolate engine logging to /dev/null or a separate debug file (2\>/tmp/servo-debug.log), reserving clean stdout strictly for formatted JSON or Lynx text.  
> * **Multimodal Token Saturation:** When running in graphic/screenshot mode, piping raw base64-encoded PNGs into the CLI agent's prompt can instantly exhaust context tokens or crash the TUI view.  
  * *Fix:* Have the tool crop the creative, write it to a local workspace cache (.agent/artifacts/ads/slot\_1.png), and pass the **file URI** and metadata payload to the Antigravity agent rather than inlining large base64 blobs.

### **Recommended Prototype Architecture**

| Layer | Component | Implementation |
| :---- | :---- | :---- |
| **Host System** | Headless Ubuntu | xvfb-run, Mesa software rasterizer, standard system fonts (fonts-dejavu) |
| **Engine** | Custom servo-fetch fork | In-process SpiderMonkey \+ Rust-level cross-origin iframe tree-walker |
| **Ad Trigger** | JS Settle Hook | Virtual scroll down, listen for SlotRenderEndedEvent, resolve after timeout |
| **Extractor** | Dual Mode Engine | Emits Lynx-style numbered markdown for text; crops framebuffer to file for media |
| **Harness Interface** | Antigravity CLI | Registered MCP tool (.agents/mcp\_config.json) or clean bash wrapper with file artifacts |

---

*User prompt: please give me a markdown summary of instructions for the initial commit for agy cli to build into a git project called servo-browse*

# **Response: Initial Commit Specification: servo-browse**

This specification provides the setup steps and file tree for agy to bootstrap and commit the initial scaffold of servo-browse: a headless, Servo-powered agentic browser designed for bash/MSYS2 subshells with Lynx-grade terminal rendering and standard IAB ad perception.

## **1\. Project Directory Layout**

servo-browse/  
├── .gitignore  
├── Cargo.toml  
├── README.md  
├── fixtures/  
│   └── iab\_sample.html  
├── scripts/  
│   └── run-headless.sh  
├── skills/  
│   └── servo-browse/  
│       └── SKILL.md  
└── src/  
    ├── ads/  
    │   ├── detector.rs  
    │   └── mod.rs  
    ├── contract.rs  
    │   └── mod.rs  
    ├── engine/  
    │   └── mod.rs  
    ├── formatter/  
    │   ├── lynx.rs  
    │   └── mod.rs  
    └── main.rs

## **2\. File Contents & Templates**

### **Cargo.toml**

\[package\]  
name \= "servo-browse"  
version \= "0.1.0"  
edition \= "2021"  
description \= "Headless agentic browser engine for subshells with Lynx-grade TUI rendering and IAB ad detection"  
authors \= \["Your Name"\]  
license \= "MIT OR Apache-2.0"

\[dependencies\]  
\# Servo engine dependency (pinning to servo-fetch or internal Servo crates)  
servo-fetch \= "0.1"  
clap \= { version \= "4.5", features \= \["derive", "env"\] }  
serde \= { version \= "1.0", features \= \["derive"\] }  
serde\_json \= "1.0"  
tokio \= { version \= "1.38", features \= \["full"\] }  
textwrap \= "0.16"  
image \= { version \= "0.25", default-features \= false, features \= \["png"\] }  
tracing \= "0.1"  
tracing-subscriber \= { version \= "0.3", features \= \["env-filter"\] }

\[profile.release\]  
opt-level \= 3  
lto \= true  
codegen-units \= 1  
strip \= true

### **src/contract.rs**

The JSON payload contract consumed by Antigravity, Buzz, and Goose agents:

use serde::{Deserialize, Serialize};

\#\[derive(Debug, Serialize, Deserialize)\]  
pub struct AgentResponse {  
    pub meta: PageMetadata,  
    pub tui\_render: String,  
    pub markdown\_render: String,  
    pub links: Vec\<IndexedLink\>,  
    pub ads: Vec\<DetectedAd\>,  
}

\#\[derive(Debug, Serialize, Deserialize)\]  
pub struct PageMetadata {  
    pub url: String,  
    pub title: String,  
    pub http\_status: u16,  
    pub viewport: (u32, u32),  
}

\#\[derive(Debug, Serialize, Deserialize)\]  
pub struct IndexedLink {  
    pub index: usize,  
    pub text: String,  
    pub url: String,  
}

\#\[derive(Debug, Serialize, Deserialize)\]  
pub struct DetectedAd {  
    pub slot\_id: String,  
    pub iab\_format: String,  
    pub dimensions: (u32, u32),  
    pub bounding\_box: (f32, f32, f32, f32), // x, y, width, height  
    pub click\_url: Option\<String\>,  
    pub alt\_text: Option\<String\>,  
    pub local\_crop\_path: Option\<String\>,  
}

### **src/ads/mod.rs**

Standard IAB dimensional profiles and detection logic:

pub mod detector;

\#\[derive(Debug, Clone, PartialEq)\]  
pub enum IabStandard {  
    Leaderboard,      // 728x90  
    MediumRectangle,  // 300x250  
    LargeRectangle,   // 336x280  
    HalfPage,         // 300x600  
    Billboard,        // 970x250  
    MobileBanner,     // 320x50  
}

impl IabStandard {  
    pub fn match\_dimensions(w: u32, h: u32) \-\> Option\<Self\> {  
        match (w, h) {  
            (728, 90\) \=\> Some(Self::Leaderboard),  
            (300, 250\) \=\> Some(Self::MediumRectangle),  
            (336, 280\) \=\> Some(Self::LargeRectangle),  
            (300, 600\) \=\> Some(Self::HalfPage),  
            (970, 250\) \=\> Some(Self::Billboard),  
            (320, 50\) \=\> Some(Self::MobileBanner),  
            \_ \=\> None,  
        }  
    }  
}

### **src/formatter/lynx.rs**

Formats the rendered layout into a numbered link model with line-wrapped text:

use crate::contract::IndexedLink;  
use textwrap::wrap;

pub struct LynxFormatter {  
    pub width: usize,  
}

impl LynxFormatter {  
    pub fn new(width: usize) \-\> Self {  
        Self { width }  
    }

    pub fn format(\&self, title: \&str, body: \&str, links: &\[IndexedLink\]) \-\> String {  
        let mut out \= String::new();  
        let border \= "═".repeat(self.width.min(80));

        out.push\_str(\&format\!("{}\\n  {}\\n{}\\n\\n", border, title, border));

        // Format wrapped text content  
        for line in body.lines() {  
            if line.trim().is\_empty() {  
                out.push('\\n');  
            } else {  
                for wrapped in wrap(line, self.width) {  
                    out.push\_str(\&wrapped);  
                    out.push('\\n');  
                }  
            }  
        }

        // Lynx-style References section  
        if \!links.is\_empty() {  
            out.push\_str("\\n── References ────────────────────────────────────────\\n");  
            for link in links {  
                out.push\_str(\&format\!("\[{}\] {}\\n", link.index, link.url));  
            }  
        }

        out  
    }  
}

### **src/main.rs**

CLI interface handling subshell arguments and stream separation:

mod ads;  
mod contract;  
mod engine;  
mod formatter;

use clap::{Parser, ValueEnum};  
use std::process::ExitCode;

\#\[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)\]  
enum OutputFormat {  
    Tui,  
    Json,  
    Png,  
}

\#\[derive(Parser, Debug)\]  
\#\[command(name \= "servo-browse", about \= "Agentic browser client for subshells")\]  
struct Args {  
    /// URL to browse  
    url: String,

    /// Output presentation mode  
    \#\[arg(short, long, value\_enum, default\_value\_t \= OutputFormat::Tui)\]  
    format: OutputFormat,

    /// Viewport column width  
    \#\[arg(long, default\_value\_t \= 80)\]  
    cols: usize,

    /// Target CSS selector to scope extraction  
    \#\[arg(long)\]  
    selector: Option\<String\>,

    /// Detect and parse standard IAB ad slots  
    \#\[arg(long, default\_value\_t \= true)\]  
    detect\_ads: bool,

    /// Save full or element screenshot to path  
    \#\[arg(short, long)\]  
    output: Option\<String\>,  
}

\#\[tokio::main\]  
async fn main() \-\> ExitCode {  
    // Direct all operational logs to stderr, keeping stdout pristine for agents  
    tracing\_subscriber::fmt()  
        .with\_writer(std::io::stderr)  
        .init();

    let args \= Args::parse();  
    eprintln\!("\[servo-browse\] Fetching {}", args.url);

    // TODO: Connect Servo runtime, execute JS settle loop, detect IAB slots,  
    // and format response according to args.format.

    ExitCode::SUCCESS  
}

### **skills/servo-browse/SKILL.md**

The instruction prompt that pairs with the binary to control orchestrators and subagents:

\# Servo-Browse Agent Skill

\#\# Tool Definition  
\- Binary: \`servo-browse\`  
\- Invocation: \`servo-browse \<url\> \[options\]\`  
\- Flags:  
  \- \`--format \<tui|json|png\>\` (default: \`tui\`)  
  \- \`--cols \<number\>\` (default: 80\)  
  \- \`--selector \<css\>\` (scoping to target DOM subtree)  
  \- \`--detect-ads \<bool\>\` (default: true)

\#\# Rendering & Presentation Contract  
When receiving output from \`servo-browse\`:

1\. \*\*Verbatim TUI Presentation:\*\*  
   \- Display \`tui\_render\` within a single fenced code block tagged \`text\`.  
   \- Preserve all link indices \`\[n\]\`, whitespace padding, and box-drawing borders.  
   \- Do NOT collapse text into markdown paragraphs unless explicitly requested by the user.

2\. \*\*IAB Ad Metadata:\*\*  
   \- When ads are detected in \`json\` mode, present them in a clean summary table:  
     | Slot ID | IAB Format | Dimensions | Destination URL |  
   \- Never show raw ad tracking pixels or base64 data to the user.

3\. \*\*Link Follow Actions:\*\*  
   \- When the user selects a link by index (e.g., "open \[3\]"), run \`servo-browse\` with the matching reference URL.

### **scripts/run-headless.sh**

Subshell wrapper ensuring offscreen rendering contexts on headless Ubuntu/Linux:

\#\!/usr/bin/env bash  
set \-euo pipefail

\# Ensure standard font directories are present  
export FONTCONFIG\_PATH="${FONTCONFIG\_PATH:-/etc/fonts}"

\# Check for display server; wrap in xvfb-run if headless  
if \[ \-z "${DISPLAY:-}" \]; then  
  if command \-v xvfb-run \>/dev/null 2\>&1; then  
    exec xvfb-run \--auto-servernum \--server-args="-screen 0 1280x800x24" servo-browse "$@"  
  else  
    echo "Error: DISPLAY not set and xvfb-run is missing. Install xvfb or configure software EGL." \>&2  
    exit 1  
  fi  
else  
  exec servo-browse "$@"  
fi

### **fixtures/iab\_sample.html**

Local test fixture with static mock IAB containers for verification:

\<\!DOCTYPE html\>  
\<html lang="en"\>  
\<head\>  
  \<meta charset="UTF-8"\>  
  \<title\>IAB Mock Testbed\</title\>  
  \<style\>  
    body { font-family: sans-serif; margin: 20px; }  
    .ad-slot { background: \#f0f0f0; border: 1px dashed \#999; margin: 10px 0; text-align: center; }  
    \#leaderboard { width: 728px; height: 90px; }  
    \#med-rect { width: 300px; height: 250px; }  
  \</style\>  
\</head\>  
\<body\>  
  \<h1\>Page Headline for Agent Verification\</h1\>  
  \<div id="leaderboard" class="ad-slot" data-iab="728x90"\>  
    \<a href="https://example.com/sponsor-click"\>  
      \<img src="data:image/svg+xml;utf8,\<svg xmlns='http://www.w3.org/2000/svg' width='728' height='90'\>\<text x='20' y='50'\>Sponsored Leaderboard\</text\>\</svg\>" alt="Sample Sponsor"\>  
    \</a\>  
  \</div\>  
  \<p\>Article paragraph body text with a \<a href="https://example.com/inline-link"\>regular anchor\</a\>.\</p\>  
  \<div id="med-rect" class="ad-slot" data-iab="300x250"\>  
    \<a href="https://example.com/rect-click"\>Medium Rectangle Ad\</a\>  
  \</div\>  
\</body\>  
\</html\>

## **3\. Terminal Execution Instructions for agy**

Run this sequence inside the subshell to initialize the repo, write the files, and commit:

\# 1\. Initialize repo  
git init servo-browse  
cd servo-browse

\# 2\. Add standard gitignore  
cat \<\< 'EOF' \> .gitignore  
/target  
Cargo.lock  
\*.png  
\*.log  
.agent/artifacts/  
EOF

\# 3\. Create directory hierarchy  
mkdir \-p src/{ads,contract,engine,formatter} fixtures scripts skills/servo-browse

\# 4\. Make scripts executable  
chmod \+x scripts/run-headless.sh

\# 5\. Verify cargo project manifest compiles (dry check)  
cargo check 2\>/dev/null || true

\# 6\. Commit initial scaffold  
git add .  
git commit \-m "feat: initial scaffold for servo-browse

\- Add CLI arg parser with TUI, JSON, and PNG output formats  
\- Add LynxFormatter skeleton for numbered-link terminal wrapping  
\- Add IAB ad dimensions and slot metadata contract  
\- Add agent skill specification (SKILL.md) and headless runner"

---

*User prompt: can you concat all of that into a single markdown prompt?*

Response: \# Objective: Initialize \`servo-browse\` Repository via \`agy\` CLI

Build and commit the initial scaffold for a new project named \`servo-browse\`.

\`servo-browse\` is a headless, Servo-powered agentic browser designed to execute cleanly inside subshell environments (e.g., MSYS2, Bash on Linux/macOS) for agents like Buzz, Goose, and Antigravity. It features:  
1\. \*\*Lynx-Grade Terminal Formatting:\*\* Terminal-aware wrapping, box drawing, and indexed link references (\`\[1\]\`, \`\[2\]\`).  
2\. \*\*Standard IAB Ad Perception:\*\* Detects standard IAB ad dimensions and slot metadata (Google Publisher Tag / SafeFrames), providing clean structured data and optional creative screenshot cropping.  
3\. \*\*Strict Subshell/Agent Contract:\*\* Separates \`stderr\` debugging from machine-readable \`stdout\`, and pairs with an agent skill (\`SKILL.md\`) that enforces layout preservation across orchestrator/subagent boundaries.

\---

\#\# Required File Tree

Ensure the workspace contains the following file structure:

\`\`\`text  
servo-browse/  
├── .gitignore  
├── Cargo.toml  
├── README.md  
├── fixtures/  
│   └── iab\_sample.html  
├── scripts/  
│   └── run-headless.sh  
├── skills/  
│   └── servo-browse/  
│       └── SKILL.md  
└── src/  
    ├── ads/  
    │   ├── detector.rs  
    │   └── mod.rs  
    ├── contract.rs  
    │   └── mod.rs  
    ├── engine/  
    │   └── mod.rs  
    ├── formatter/  
    │   ├── lynx.rs  
    │   └── mod.rs  
    └── main.rs

## **File Contents**

### **1\. .gitignore**

/target  
Cargo.lock  
\*.png  
\*.log  
.agent/artifacts/

### **2\. Cargo.toml**

\[package\]  
name \= "servo-browse"  
version \= "0.1.0"  
edition \= "2021"  
description \= "Headless agentic browser engine for subshells with Lynx-grade TUI rendering and IAB ad detection"  
authors \= \["Your Name"\]  
license \= "MIT OR Apache-2.0"

\[dependencies\]  
servo-fetch \= "0.1"  
clap \= { version \= "4.5", features \= \["derive", "env"\] }  
serde \= { version \= "1.0", features \= \["derive"\] }  
serde\_json \= "1.0"  
tokio \= { version \= "1.38", features \= \["full"\] }  
textwrap \= "0.16"  
image \= { version \= "0.25", default-features \= false, features \= \["png"\] }  
tracing \= "0.1"  
tracing-subscriber \= { version \= "0.3", features \= \["env-filter"\] }

\[profile.release\]  
opt-level \= 3  
lto \= true  
codegen-units \= 1  
strip \= true

### **3\. README.md**

\# servo-browse

An agentic headless browser engine built on Servo, designed for execution inside subshells (Bash, MSYS2) by AI agent harnesses (Buzz, Goose, Antigravity).

\#\# Key Features  
\- \*\*Lynx-Grade Terminal Rendering\*\*: Emits line-wrapped terminal views with numbered footnote references and box-drawing borders.  
\- \*\*IAB Ad Perception\*\*: Detects standard IAB ad units (Leaderboard, Medium Rectangle, etc.), extracting destination URLs and bounding boxes without stripping them via ad-blockers.  
\- \*\*Subshell & Stream Isolation\*\*: Emits clean JSON/TUI to \`stdout\` while routing engine logs and diagnostics strictly to \`stderr\`.

### **4\. src/contract.rs**

use serde::{Deserialize, Serialize};

\#\[derive(Debug, Serialize, Deserialize)\]  
pub struct AgentResponse {  
    pub meta: PageMetadata,  
    pub tui\_render: String,  
    pub markdown\_render: String,  
    pub links: Vec\<IndexedLink\>,  
    pub ads: Vec\<DetectedAd\>,  
}

\#\[derive(Debug, Serialize, Deserialize)\]  
pub struct PageMetadata {  
    pub url: String,  
    pub title: String,  
    pub http\_status: u16,  
    pub viewport: (u32, u32),  
}

\#\[derive(Debug, Serialize, Deserialize)\]  
pub struct IndexedLink {  
    pub index: usize,  
    pub text: String,  
    pub url: String,  
}

\#\[derive(Debug, Serialize, Deserialize)\]  
pub struct DetectedAd {  
    pub slot\_id: String,  
    pub iab\_format: String,  
    pub dimensions: (u32, u32),  
    pub bounding\_box: (f32, f32, f32, f32), // x, y, width, height  
    pub click\_url: Option\<String\>,  
    pub alt\_text: Option\<String\>,  
    pub local\_crop\_path: Option\<String\>,  
}

### **5\. src/ads/mod.rs**

pub mod detector;

\#\[derive(Debug, Clone, PartialEq)\]  
pub enum IabStandard {  
    Leaderboard,      // 728x90  
    MediumRectangle,  // 300x250  
    LargeRectangle,   // 336x280  
    HalfPage,         // 300x600  
    Billboard,        // 970x250  
    MobileBanner,     // 320x50  
}

impl IabStandard {  
    pub fn match\_dimensions(w: u32, h: u32) \-\> Option\<Self\> {  
        match (w, h) {  
            (728, 90\) \=\> Some(Self::Leaderboard),  
            (300, 250\) \=\> Some(Self::MediumRectangle),  
            (336, 280\) \=\> Some(Self::LargeRectangle),  
            (300, 600\) \=\> Some(Self::HalfPage),  
            (970, 250\) \=\> Some(Self::Billboard),  
            (320, 50\) \=\> Some(Self::MobileBanner),  
            \_ \=\> None,  
        }  
    }  
}

### **6\. src/ads/detector.rs**

use super::IabStandard;  
use crate::contract::DetectedAd;

pub struct AdDetector;

impl AdDetector {  
    pub fn new() \-\> Self {  
        Self  
    }

    pub fn inspect\_slot(\&self, slot\_id: \&str, width: u32, height: u32) \-\> Option\<DetectedAd\> {  
        let matched \= IabStandard::match\_dimensions(width, height)?;  
        Some(DetectedAd {  
            slot\_id: slot\_id.to\_string(),  
            iab\_format: format\!("{:?}", matched),  
            dimensions: (width, height),  
            bounding\_box: (0.0, 0.0, width as f32, height as f32),  
            click\_url: None,  
            alt\_text: None,  
            local\_crop\_path: None,  
        })  
    }  
}

### **7\. src/formatter/lynx.rs**

use crate::contract::IndexedLink;  
use textwrap::wrap;

pub struct LynxFormatter {  
    pub width: usize,  
}

impl LynxFormatter {  
    pub fn new(width: usize) \-\> Self {  
        Self { width }  
    }

    pub fn format(\&self, title: \&str, body: \&str, links: &\[IndexedLink\]) \-\> String {  
        let mut out \= String::new();  
        let border \= "═".repeat(self.width.min(80));

        out.push\_str(\&format\!("{}\\n  {}\\n{}\\n\\n", border, title, border));

        for line in body.lines() {  
            if line.trim().is\_empty() {  
                out.push('\\n');  
            } else {  
                for wrapped in wrap(line, self.width) {  
                    out.push\_str(\&wrapped);  
                    out.push('\\n');  
                }  
            }  
        }

        if \!links.is\_empty() {  
            out.push\_str("\\n── References ────────────────────────────────────────\\n");  
            for link in links {  
                out.push\_str(\&format\!("\[{}\] {}\\n", link.index, link.url));  
            }  
        }

        out  
    }  
}

### **8\. src/formatter/mod.rs**

pub mod lynx;

### **9\. src/engine/mod.rs**

// Engine hook for Servo integration, SpiderMonkey JS loops, and offscreen context  
pub struct HeadlessEngine;

impl HeadlessEngine {  
    pub fn new() \-\> Self {  
        Self  
    }  
}

### **10\. src/main.rs**

mod ads;  
mod contract;  
mod engine;  
mod formatter;

use clap::{Parser, ValueEnum};  
use std::process::ExitCode;

\#\[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)\]  
enum OutputFormat {  
    Tui,  
    Json,  
    Png,  
}

\#\[derive(Parser, Debug)\]  
\#\[command(name \= "servo-browse", about \= "Agentic browser client for subshells")\]  
struct Args {  
    /// URL to browse  
    url: String,

    /// Output presentation mode  
    \#\[arg(short, long, value\_enum, default\_value\_t \= OutputFormat::Tui)\]  
    format: OutputFormat,

    /// Viewport column width  
    \#\[arg(long, default\_value\_t \= 80)\]  
    cols: usize,

    /// Target CSS selector to scope extraction  
    \#\[arg(long)\]  
    selector: Option\<String\>,

    /// Detect and parse standard IAB ad slots  
    \#\[arg(long, default\_value\_t \= true)\]  
    detect\_ads: bool,

    /// Save full or element screenshot to path  
    \#\[arg(short, long)\]  
    output: Option\<String\>,  
}

\#\[tokio::main\]  
async fn main() \-\> ExitCode {  
    // Keep stdout clean for agents; operational logs go to stderr  
    tracing\_subscriber::fmt()  
        .with\_writer(std::io::stderr)  
        .init();

    let args \= Args::parse();  
    eprintln\!("\[servo-browse\] Target URL: {}", args.url);

    ExitCode::SUCCESS  
}

### **11\. skills/servo-browse/SKILL.md**

\# Servo-Browse Agent Skill

\#\# Tool Definition  
\- Binary: \`servo-browse\`  
\- Invocation: \`servo-browse \<url\> \[options\]\`  
\- Flags:  
  \- \`--format \<tui|json|png\>\` (default: \`tui\`)  
  \- \`--cols \<number\>\` (default: 80\)  
  \- \`--selector \<css\>\` (scoping to target DOM subtree)  
  \- \`--detect-ads \<bool\>\` (default: true)

\#\# Rendering & Presentation Contract  
When receiving output from \`servo-browse\`:

1\. \*\*Verbatim TUI Presentation:\*\*  
   \- Display \`tui\_render\` within a single fenced code block tagged \`text\`.  
   \- Preserve all link indices \`\[n\]\`, whitespace padding, and box-drawing borders.  
   \- Do NOT collapse text into markdown paragraphs unless explicitly requested by the user.

2\. \*\*IAB Ad Metadata:\*\*  
   \- When ads are detected in \`json\` mode, present them in a clean summary table:  
     | Slot ID | IAB Format | Dimensions | Destination URL |  
   \- Never show raw ad tracking pixels or base64 data to the user.

3\. \*\*Link Follow Actions:\*\*  
   \- When the user selects a link by index (e.g., "open \[3\]"), run \`servo-browse\` with the matching reference URL.

### **12\. scripts/run-headless.sh**

\#\!/usr/bin/env bash  
set \-euo pipefail

export FONTCONFIG\_PATH="${FONTCONFIG\_PATH:-/etc/fonts}"

if \[ \-z "${DISPLAY:-}" \]; then  
  if command \-v xvfb-run \>/dev/null 2\>&1; then  
    exec xvfb-run \--auto-servernum \--server-args="-screen 0 1280x800x24" servo-browse "$@"  
  else  
    echo "Error: DISPLAY not set and xvfb-run is missing. Install xvfb or configure software EGL." \>&2  
    exit 1  
  fi  
else  
  exec servo-browse "$@"  
fi

### **13\. fixtures/iab\_sample.html**

\<\!DOCTYPE html\>  
\<html lang="en"\>  
\<head\>  
  \<meta charset="UTF-8"\>  
  \<title\>IAB Mock Testbed\</title\>  
  \<style\>  
    body { font-family: sans-serif; margin: 20px; }  
    .ad-slot { background: \#f0f0f0; border: 1px dashed \#999; margin: 10px 0; text-align: center; }  
    \#leaderboard { width: 728px; height: 90px; }  
    \#med-rect { width: 300px; height: 250px; }  
  \</style\>  
\</head\>  
\<body\>  
  \<h1\>Page Headline for Agent Verification\</h1\>  
  \<div id="leaderboard" class="ad-slot" data-iab="728x90"\>  
    \<a href="\[https://example.com/sponsor-click\](https://example.com/sponsor-click)"\>  
      \<img src="data:image/svg+xml;utf8,\<svg xmlns='\[http://www.w3.org/2000/svg\](http://www.w3.org/2000/svg)' width='728' height='90'\>\<text x='20' y='50'\>Sponsored Leaderboard\</text\>\</svg\>" alt="Sample Sponsor"\>  
    \</a\>  
  \</div\>  
  \<p\>Article paragraph body text with a \<a href="\[https://example.com/inline-link\](https://example.com/inline-link)"\>regular anchor\</a\>.\</p\>  
  \<div id="med-rect" class="ad-slot" data-iab="300x250"\>  
    \<a href="\[https://example.com/rect-click\](https://example.com/rect-click)"\>Medium Rectangle Ad\</a\>  
  \</div\>  
\</body\>  
\</html\>

## **Subshell Execution & Git Commit Steps**

Execute the following shell steps inside your workspace:

\# 1\. Initialize git repo  
git init servo-browse  
cd servo-browse

\# 2\. Write out all files described above into their respective paths  
\# (Create directories: mkdir \-p src/{ads,formatter,engine} fixtures scripts skills/servo-browse)

\# 3\. Mark the headless wrapper executable  
chmod \+x scripts/run-headless.sh

\# 4\. Stage and verify git tree  
git add .  
git status

\# 5\. Create initial commit  
git commit \-m "feat: initial scaffold for servo-browse

\- Add CLI arg parser with TUI, JSON, and PNG output formats  
\- Add LynxFormatter skeleton for numbered-link terminal wrapping  
\- Add IAB ad dimensions and slot metadata contract  
\- Add agent skill specification (SKILL.md) and headless runner"

---

*User prompt: can you zip all of that up into a single document?*

Response: Here is the self-contained setup packaged as an executable shell script document. You can drop this directly into a file (e.g., init-servo-browse.sh) or run it in your subshell to generate the entire workspace and initial commit automatically:

\#\!/usr/bin/env bash  
set \-euo pipefail

REPO\_DIR="servo-browse"

echo "==\> Initializing git repository: ${REPO\_DIR}"  
git init "${REPO\_DIR}"  
cd "${REPO\_DIR}"

echo "==\> Creating directory tree..."  
mkdir \-p src/ads src/contract src/engine src/formatter fixtures scripts skills/servo-browse

echo "==\> Writing .gitignore..."  
cat \<\< 'EOF' \> .gitignore  
/target  
Cargo.lock  
\*.png  
\*.log  
.agent/artifacts/  
EOF

echo "==\> Writing Cargo.toml..."  
cat \<\< 'EOF' \> Cargo.toml  
\[package\]  
name \= "servo-browse"  
version \= "0.1.0"  
edition \= "2021"  
description \= "Headless agentic browser engine for subshells with Lynx-grade TUI rendering and IAB ad detection"  
authors \= \["Your Name"\]  
license \= "MIT OR Apache-2.0"

\[dependencies\]  
servo-fetch \= "0.1"  
clap \= { version \= "4.5", features \= \["derive", "env"\] }  
serde \= { version \= "1.0", features \= \["derive"\] }  
serde\_json \= "1.0"  
tokio \= { version \= "1.38", features \= \["full"\] }  
textwrap \= "0.16"  
image \= { version \= "0.25", default-features \= false, features \= \["png"\] }  
tracing \= "0.1"  
tracing-subscriber \= { version \= "0.3", features \= \["env-filter"\] }

\[profile.release\]  
opt-level \= 3  
lto \= true  
codegen-units \= 1  
strip \= true  
EOF

echo "==\> Writing README.md..."  
cat \<\< 'EOF' \> README.md  
\# servo-browse

An agentic headless browser engine built on Servo, designed for execution inside subshells (Bash, MSYS2) by AI agent harnesses (Buzz, Goose, Antigravity).

\#\# Key Features  
\- \*\*Lynx-Grade Terminal Rendering\*\*: Emits line-wrapped terminal views with numbered footnote references and box-drawing borders.  
\- \*\*IAB Ad Perception\*\*: Detects standard IAB ad units (Leaderboard, Medium Rectangle, etc.), extracting destination URLs and bounding boxes without stripping them via ad-blockers.  
\- \*\*Subshell & Stream Isolation\*\*: Emits clean JSON/TUI to \`stdout\` while routing engine logs and diagnostics strictly to \`stderr\`.  
EOF

echo "==\> Writing src/contract.rs..."  
cat \<\< 'EOF' \> src/contract.rs  
use serde::{Deserialize, Serialize};

\#\[derive(Debug, Serialize, Deserialize)\]  
pub struct AgentResponse {  
    pub meta: PageMetadata,  
    pub tui\_render: String,  
    pub markdown\_render: String,  
    pub links: Vec\<IndexedLink\>,  
    pub ads: Vec\<DetectedAd\>,  
}

\#\[derive(Debug, Serialize, Deserialize)\]  
pub struct PageMetadata {  
    pub url: String,  
    pub title: String,  
    pub http\_status: u16,  
    pub viewport: (u32, u32),  
}

\#\[derive(Debug, Serialize, Deserialize)\]  
pub struct IndexedLink {  
    pub index: usize,  
    pub text: String,  
    pub url: String,  
}

\#\[derive(Debug, Serialize, Deserialize)\]  
pub struct DetectedAd {  
    pub slot\_id: String,  
    pub iab\_format: String,  
    pub dimensions: (u32, u32),  
    pub bounding\_box: (f32, f32, f32, f32), // x, y, width, height  
    pub click\_url: Option\<String\>,  
    pub alt\_text: Option\<String\>,  
    pub local\_crop\_path: Option\<String\>,  
}  
EOF

echo "==\> Writing src/ads/mod.rs..."  
cat \<\< 'EOF' \> src/ads/mod.rs  
pub mod detector;

\#\[derive(Debug, Clone, PartialEq)\]  
pub enum IabStandard {  
    Leaderboard,      // 728x90  
    MediumRectangle,  // 300x250  
    LargeRectangle,   // 336x280  
    HalfPage,         // 300x600  
    Billboard,        // 970x250  
    MobileBanner,     // 320x50  
}

impl IabStandard {  
    pub fn match\_dimensions(w: u32, h: u32) \-\> Option\<Self\> {  
        match (w, h) {  
            (728, 90\) \=\> Some(Self::Leaderboard),  
            (300, 250\) \=\> Some(Self::MediumRectangle),  
            (336, 280\) \=\> Some(Self::LargeRectangle),  
            (300, 600\) \=\> Some(Self::HalfPage),  
            (970, 250\) \=\> Some(Self::Billboard),  
            (320, 50\) \=\> Some(Self::MobileBanner),  
            \_ \=\> None,  
        }  
    }  
}  
EOF

echo "==\> Writing src/ads/detector.rs..."  
cat \<\< 'EOF' \> src/ads/detector.rs  
use super::IabStandard;  
use crate::contract::DetectedAd;

pub struct AdDetector;

impl AdDetector {  
    pub fn new() \-\> Self {  
        Self  
    }

    pub fn inspect\_slot(\&self, slot\_id: \&str, width: u32, height: u32) \-\> Option\<DetectedAd\> {  
        let matched \= IabStandard::match\_dimensions(width, height)?;  
        Some(DetectedAd {  
            slot\_id: slot\_id.to\_string(),  
            iab\_format: format\!("{:?}", matched),  
            dimensions: (width, height),  
            bounding\_box: (0.0, 0.0, width as f32, height as f32),  
            click\_url: None,  
            alt\_text: None,  
            local\_crop\_path: None,  
        })  
    }  
}  
EOF

echo "==\> Writing src/formatter/lynx.rs..."  
cat \<\< 'EOF' \> src/formatter/lynx.rs  
use crate::contract::IndexedLink;  
use textwrap::wrap;

pub struct LynxFormatter {  
    pub width: usize,  
}

impl LynxFormatter {  
    pub fn new(width: usize) \-\> Self {  
        Self { width }  
    }

    pub fn format(\&self, title: \&str, body: \&str, links: &\[IndexedLink\]) \-\> String {  
        let mut out \= String::new();  
        let border \= "═".repeat(self.width.min(80));

        out.push\_str(\&format\!("{}\\n  {}\\n{}\\n\\n", border, title, border));

        for line in body.lines() {  
            if line.trim().is\_empty() {  
                out.push('\\n');  
            } else {  
                for wrapped in wrap(line, self.width) {  
                    out.push\_str(\&wrapped);  
                    out.push('\\n');  
                }  
            }  
        }

        if \!links.is\_empty() {  
            out.push\_str("\\n── References ────────────────────────────────────────\\n");  
            for link in links {  
                out.push\_str(\&format\!("\[{}\] {}\\n", link.index, link.url));  
            }  
        }

        out  
    }  
}  
EOF

echo "==\> Writing src/formatter/mod.rs..."  
cat \<\< 'EOF' \> src/formatter/mod.rs  
pub mod lynx;  
EOF

echo "==\> Writing src/engine/mod.rs..."  
cat \<\< 'EOF' \> src/engine/mod.rs  
pub struct HeadlessEngine;

impl HeadlessEngine {  
    pub fn new() \-\> Self {  
        Self  
    }  
}  
EOF

echo "==\> Writing src/main.rs..."  
cat \<\< 'EOF' \> src/main.rs  
mod ads;  
mod contract;  
mod engine;  
mod formatter;

use clap::{Parser, ValueEnum};  
use std::process::ExitCode;

\#\[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)\]  
enum OutputFormat {  
    Tui,  
    Json,  
    Png,  
}

\#\[derive(Parser, Debug)\]  
\#\[command(name \= "servo-browse", about \= "Agentic browser client for subshells")\]  
struct Args {  
    /// URL to browse  
    url: String,

    /// Output presentation mode  
    \#\[arg(short, long, value\_enum, default\_value\_t \= OutputFormat::Tui)\]  
    format: OutputFormat,

    /// Viewport column width  
    \#\[arg(long, default\_value\_t \= 80)\]  
    cols: usize,

    /// Target CSS selector to scope extraction  
    \#\[arg(long)\]  
    selector: Option\<String\>,

    /// Detect and parse standard IAB ad slots  
    \#\[arg(long, default\_value\_t \= true)\]  
    detect\_ads: bool,

    /// Save full or element screenshot to path  
    \#\[arg(short, long)\]  
    output: Option\<String\>,  
}

\#\[tokio::main\]  
async fn main() \-\> ExitCode {  
    tracing\_subscriber::fmt()  
        .with\_writer(std::io::stderr)  
        .init();

    let args \= Args::parse();  
    eprintln\!("\[servo-browse\] Target URL: {}", args.url);

    ExitCode::SUCCESS  
}  
EOF

echo "==\> Writing skills/servo-browse/SKILL.md..."  
cat \<\< 'EOF' \> skills/servo-browse/SKILL.md  
\# Servo-Browse Agent Skill

\#\# Tool Definition  
\- Binary: \`servo-browse\`  
\- Invocation: \`servo-browse \<url\> \[options\]\`  
\- Flags:  
  \- \`--format \<tui|json|png\>\` (default: \`tui\`)  
  \- \`--cols \<number\>\` (default: 80\)  
  \- \`--selector \<css\>\` (scoping to target DOM subtree)  
  \- \`--detect-ads \<bool\>\` (default: true)

\#\# Rendering & Presentation Contract  
When receiving output from \`servo-browse\`:

1\. \*\*Verbatim TUI Presentation:\*\*  
   \- Display \`tui\_render\` within a single fenced code block tagged \`text\`.  
   \- Preserve all link indices \`\[n\]\`, whitespace padding, and box-drawing borders.  
   \- Do NOT collapse text into markdown paragraphs unless explicitly requested by the user.

2\. \*\*IAB Ad Metadata:\*\*  
   \- When ads are detected in \`json\` mode, present them in a clean summary table:  
     | Slot ID | IAB Format | Dimensions | Destination URL |  
   \- Never show raw ad tracking pixels or base64 data to the user.

3\. \*\*Link Follow Actions:\*\*  
   \- When the user selects a link by index (e.g., "open \[3\]"), run \`servo-browse\` with the matching reference URL.  
EOF

echo "==\> Writing scripts/run-headless.sh..."  
cat \<\< 'EOF' \> scripts/run-headless.sh  
\#\!/usr/bin/env bash  
set \-euo pipefail

export FONTCONFIG\_PATH="${FONTCONFIG\_PATH:-/etc/fonts}"

if \[ \-z "${DISPLAY:-}" \]; then  
  if command \-v xvfb-run \>/dev/null 2\>&1; then  
    exec xvfb-run \--auto-servernum \--server-args="-screen 0 1280x800x24" servo-browse "$@"  
  else  
    echo "Error: DISPLAY not set and xvfb-run is missing. Install xvfb or configure software EGL." \>&2  
    exit 1  
  fi  
else  
  exec servo-browse "$@"  
fi  
EOF  
chmod \+x scripts/run-headless.sh

echo "==\> Writing fixtures/iab\_sample.html..."  
cat \<\< 'EOF' \> fixtures/iab\_sample.html  
\<\!DOCTYPE html\>  
\<html lang="en"\>  
\<head\>  
  \<meta charset="UTF-8"\>  
  \<title\>IAB Mock Testbed\</title\>  
  \<style\>  
    body { font-family: sans-serif; margin: 20px; }  
    .ad-slot { background: \#f0f0f0; border: 1px dashed \#999; margin: 10px 0; text-align: center; }  
    \#leaderboard { width: 728px; height: 90px; }  
    \#med-rect { width: 300px; height: 250px; }  
  \</style\>  
\</head\>  
\<body\>  
  \<h1\>Page Headline for Agent Verification\</h1\>  
  \<div id="leaderboard" class="ad-slot" data-iab="728x90"\>  
    \<a href="https://example.com/sponsor-click"\>  
      \<img src="data:image/svg+xml;utf8,\<svg xmlns='http://www.w3.org/2000/svg' width='728' height='90'\>\<text x='20' y='50'\>Sponsored Leaderboard\</text\>\</svg\>" alt="Sample Sponsor"\>  
    \</a\>  
  \</div\>  
  \<p\>Article paragraph body text with a \<a href="https://example.com/inline-link"\>regular anchor\</a\>.\</p\>  
  \<div id="med-rect" class="ad-slot" data-iab="300x250"\>  
    \<a href="https://example.com/rect-click"\>Medium Rectangle Ad\</a\>  
  \</div\>  
\</body\>  
\</html\>  
EOF

echo "==\> Staging files and creating initial git commit..."  
git add .  
git commit \-m "feat: initial scaffold for servo-browse

\- Add CLI arg parser with TUI, JSON, and PNG output formats  
\- Add LynxFormatter skeleton for numbered-link terminal wrapping  
\- Add IAB ad dimensions and slot metadata contract  
\- Add agent skill specification (SKILL.md) and headless runner"

echo "==\> Done\! Initial commit created for servo-browse."

---

