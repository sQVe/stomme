use std::process::ExitCode;

#[expect(
    clippy::print_stderr,
    reason = "the usage message is the command's output"
)]
fn main() -> ExitCode {
    eprintln!("Usage: stomme <command>\n\nNo commands are available yet.");
    ExitCode::FAILURE
}
