mod agy;
mod error;
mod input;
mod models;
mod probe;
mod prompts;
mod server;

use std::process::ExitCode;

use clap::Parser;
use rmcp::ServiceExt;
use rmcp::service::QuitReason;
use rmcp::transport::stdio;
use server::VisionServer;
use tokio::runtime::Runtime;

/// MCP server that gives visionless LLMs Gemini's eyes, ears, and video comprehension via `agy` CLI.
#[derive(Parser)]
#[command(name = "gemini-multimodal-mcp")]
struct Args {
    /// Set default Gemini model to use when not specified in tool calls
    #[arg(short, long, value_name = "MODEL")]
    model: Option<String>,
}

fn main() -> ExitCode {
    let args = Args::parse();

    if let Some(ref default_model) = args.model {
        let models = models::list_models();
        if !models.is_empty() {
            if let Err(err) = models::validate(&models, default_model) {
                eprintln!("Warning: {err}");
            }
        }
    }

    let runtime = match Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to start tokio runtime: {e}");
            return ExitCode::from(1);
        }
    };

    match runtime.block_on(serve(args.model)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("gemini-multimodal-mcp: {e}");
            ExitCode::from(1)
        }
    }
}

async fn serve(
    default_model: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (stdin, stdout) = stdio();
    let service = VisionServer::new(default_model);
    let running = service.serve((stdin, stdout)).await?;
    let reason = running.waiting().await?;
    quit_result(reason)
}

fn quit_result(reason: QuitReason) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match reason {
        QuitReason::Closed | QuitReason::Cancelled => Ok(()),
        QuitReason::JoinError(e) => Err(Box::new(e)),
        _ => Ok(()),
    }
}
