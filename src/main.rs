pub mod ads;
pub mod contract;
pub mod engine;
pub mod formatter;

use clap::{Parser, ValueEnum};
use std::process::ExitCode;
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

#[derive(Parser, Debug)]
#[command(
    name = "servo-browse",
    about = "Headless agentic browser engine for subshells with Lynx-grade TUI rendering and IAB ad detection"
)]
pub struct Args {
    /// Target URL or local file path to browse
    pub url: String,

    /// Output presentation mode
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Tui)]
    pub format: OutputFormat,

    /// Viewport column width for text wrapping
    #[arg(long, default_value_t = 80)]
    pub cols: usize,

    /// Target CSS selector to scope extraction
    #[arg(long)]
    pub selector: Option<String>,

    /// Detect and parse standard IAB ad slots
    #[arg(long, default_value_t = true)]
    pub detect_ads: bool,

    /// Save full or element screenshot to path (PNG mode)
    #[arg(short, long)]
    pub output: Option<String>,
}

#[tokio::main]
async fn main() -> ExitCode {
    // Keep stdout clean for agents: direct all operational logs and traces strictly to stderr
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();
    eprintln!("[servo-browse] Loading: {}", args.url);

    let engine = HeadlessEngine::new();
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

    let response = engine.process_page(
        &html,
        &resolved_url,
        status,
        args.cols,
        args.selector.as_deref(),
        args.detect_ads,
    );

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
            eprintln!("[servo-browse] PNG rendering requires active GPU/EGL rasterizer.");
        }
    }

    ExitCode::SUCCESS
}
