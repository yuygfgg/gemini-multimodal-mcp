mod agy;
mod error;
mod input;
mod models;
mod probe;
mod prompts;
mod server;

use std::process::ExitCode;

use rmcp::ServiceExt;
use rmcp::service::QuitReason;
use rmcp::transport::stdio;
use server::VisionServer;
use tokio::runtime::Runtime;

struct Args {
    default_model: Option<String>,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut args_iter = std::env::args().skip(1);
    let mut default_model = None;

    while let Some(arg) = args_iter.next() {
        if arg == "-h" || arg == "--help" {
            print_help();
            return Ok(None);
        } else if arg == "-m" || arg == "--model" {
            if let Some(val) = args_iter.next() {
                default_model = Some(val);
            } else {
                return Err(format!("error: option '{arg}' requires an argument"));
            }
        } else if arg.starts_with("--model=") {
            let val = arg.strip_prefix("--model=").unwrap();
            default_model = Some(val.to_string());
        } else if arg.starts_with("-m=") {
            let val = arg.strip_prefix("-m=").unwrap();
            default_model = Some(val.to_string());
        } else {
            return Err(format!(
                "error: unknown argument '{arg}'\n\n\
                 Usage: gemini-multimodal-mcp [OPTIONS]\n\n\
                 For more information, try '--help'."
            ));
        }
    }

    Ok(Some(Args { default_model }))
}

fn print_help() {
    println!(
        "gemini-multimodal-mcp\n\n\
         MCP server that gives visionless LLMs Gemini's eyes, ears, and video comprehension via `agy` CLI.\n\n\
         Usage: gemini-multimodal-mcp [OPTIONS]\n\n\
         Options:\n\
           -m, --model <MODEL>              Set default Gemini model to use when not specified in tool calls\n\
           -h, --help                       Print help information"
    );
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(Some(args)) => args,
        Ok(None) => return ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };

    if let Some(ref default_model) = args.default_model {
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

    match runtime.block_on(serve(args.default_model)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("gemini-multimodal-mcp: {e}");
            ExitCode::from(1)
        }
    }
}

async fn serve(default_model: Option<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
