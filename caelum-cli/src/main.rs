use std::process::ExitCode;

mod cli;
mod fs_resolver;

fn main() -> ExitCode {
    cli::run()
}
