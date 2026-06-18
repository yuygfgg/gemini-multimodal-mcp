use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("failed to resolve input: {0}")]
    Input(String),

    #[error("failed to decode base64 payload: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("`agy` did not respond within {0:?}")]
    Timeout(std::time::Duration),

    #[error("`agy` exited with status {status}: {stderr}")]
    AgyExit { status: i32, stderr: String },

    #[error("`agy` was killed by a signal")]
    AgySignaled,

    #[error("`agy` produced no output")]
    EmptyOutput,

    #[error(
        "media is {secs}s long which exceeds the {threshold}s limit. \
         Re-invoke with `confirm_long_media: true` to proceed anyway. \
         Long media consumes agy quota rapidly."
    )]
    LongMedia { secs: u64, threshold: u64 },

    #[error(
        "input appears to be {actual}, but this tool expects {expected}. \
         Re-invoke with `force_input_type: true` to proceed anyway."
    )]
    InputTypeMismatch {
        expected: &'static str,
        actual: &'static str,
    },

    #[error(
        "could not determine whether the input is {expected}. \
         Re-invoke with `force_input_type: true` to proceed anyway."
    )]
    UnknownInputType { expected: &'static str },

    #[error(
        "could not determine media duration before sending it to agy. \
         Re-invoke with `confirm_long_media: true` to proceed anyway. \
         Unknown-duration media may consume agy quota rapidly."
    )]
    UnknownDuration,
}
