use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match si_git_commit::cli::run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
