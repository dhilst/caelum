use std::process::ExitCode;

mod cli;
mod fs_resolver;
#[cfg(feature = "smtlib")]
mod z3_oracle;

fn main() -> ExitCode {
    cli::run()
}
