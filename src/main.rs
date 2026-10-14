pub mod ads;
pub mod contract;
pub mod engine;
pub mod formatter;

use clap::{Parser, ValueEnum};
use std::process::ExitCode;
use crate::contract::ConsentChoice;
use crate::engine::HeadlessEngine;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum OutputFormat {
    /// Lynx-grade formatted terminal view
    Tui,
    /// Machine-readable structured JSON payload
    Json,
    /// Software rasterized screenshot PNG
    Png,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum EngineChoice {
    /// Full in-process Servo engine with SpiderMonkey JS and software rendering
    Servo,
    /// Fast static HTML & DOM parser (zero-JS)
    Fast,
}

#[derive(Parser, Debug)]
#[command(
    name = "servo-browse",
    about = "Headless agentic browser engine for subshells with Lynx-grade TUI rendering and IAB ad detection"
)]
pub struct Args {
    /// Target URL or local file path to browse
    pub url: String,

    /// Browser engine mode
    #[arg(short, long, value_enum, default_value_t = EngineChoice::Servo)]
    pub engine: EngineChoice,

    /// Output presentation mode
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Tui)]
    pub format: OutputFormat,

    /// Viewport column width for text wrapping
    #[arg(long, default_value_t = 80)]
    pub cols: usize,

    /// Page load and execution timeout in seconds
    #[arg(long, default_value_t = 15)]
    pub timeout: u64,

    /// Target CSS selector to scope extraction
    #[arg(long)]
    pub selector: Option<String>,

    /// Detect and parse standard IAB ad slots
    #[arg(long, default_value_t = true)]
    pub detect_ads: bool,

    /// Save full or element screenshot to path (PNG mode)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Save cropped images of each detected ad creative
    #[arg(long)]
    pub crop_ads: bool,

    /// Output directory for ad crops
    #[arg(long, default_value = ".agent/artifacts/ads")]
    pub ads_dir: String,

    /// GDPR / Cookie Consent mode (opt-in, opt-out, ignore)
    #[arg(long, default_value = "opt-in")]
    pub consent: ConsentChoice,
}

#[tokio::main]
async fn main() -> ExitCode {
    // Install default crypto provider for rustls (required by Servo's network threads)
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Keep stdout clean for agents: direct all operational logs and traces strictly to stderr
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();
    eprintln!("[servo-browse] Target: {}", args.url);

    let engine = HeadlessEngine::new();
    let (response, screenshot) = match args.engine {
        EngineChoice::Servo => {
            let capture_png = args.format == OutputFormat::Png || args.output.is_some();
            match engine.browse_servo(
                &args.url,
                args.cols,
                args.selector.as_deref(),
                args.detect_ads,
                std::time::Duration::from_secs(args.timeout),
                capture_png,
                args.crop_ads,
                Some(&args.ads_dir),
                args.consent,
            ) {
                Ok(res) => res,
                Err(e) => {
                    eprintln!("[servo-browse] Error in Servo engine: {}", e);
                    return ExitCode::FAILURE;
                }
            }
        }
        EngineChoice::Fast => {
            let (html, resolved_url, status) = match engine.load_html(&args.url).await {
                Ok(res) => res,
                Err(e) => {
                    eprintln!("[servo-browse] Error loading {}: {}", args.url, e);
                    return ExitCode::FAILURE;
                }
            };

            if status >= 400 {
                eprintln!("[servo-browse] Warning: Server responded with HTTP status {}", status);
            }

            let resp = engine.process_page(
                &html,
                &resolved_url,
                status,
                args.cols,
                args.selector.as_deref(),
                args.detect_ads,
            );
            (resp, None)
        }
    };

    // Save screenshot if requested via --output or in PNG mode
    if let Some(ref img) = screenshot {
        let out_path = args.output.as_deref().unwrap_or("screenshot.png");
        if let Err(e) = img.save(out_path) {
            eprintln!("[servo-browse] Failed to save screenshot to {}: {}", out_path, e);
        } else {
            eprintln!("[servo-browse] Saved screenshot to: {}", out_path);
        }
    }

    match args.format {
        OutputFormat::Tui => {
            use std::io::Write;
            if let Err(e) = writeln!(std::io::stdout(), "{}", response.tui_render) {
                if e.kind() == std::io::ErrorKind::BrokenPipe {
                    return ExitCode::SUCCESS;
                }
                eprintln!("[servo-browse] Error writing stdout: {}", e);
                return ExitCode::FAILURE;
            }
        }
        OutputFormat::Json => {
            use std::io::Write;
            match serde_json::to_string_pretty(&response) {
                Ok(json) => {
                    if let Err(e) = writeln!(std::io::stdout(), "{}", json) {
                        if e.kind() == std::io::ErrorKind::BrokenPipe {
                            return ExitCode::SUCCESS;
                        }
                        eprintln!("[servo-browse] Error writing stdout: {}", e);
                        return ExitCode::FAILURE;
                    }
                }
                Err(e) => {
                    eprintln!("[servo-browse] Error serializing JSON: {}", e);
                    return ExitCode::FAILURE;
                }
            }
        }
        OutputFormat::Png => {
            if screenshot.is_none() {
                eprintln!("[servo-browse] Error: Screenshot capture is only supported with the Servo engine.");
                return ExitCode::FAILURE;
            }
        }
    }

    ExitCode::SUCCESS
}
