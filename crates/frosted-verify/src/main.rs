//! `frosted-verify`: see `--help`.

use std::process::ExitCode;

use frosted_verify::cli::{self, HELP, Invocation};

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let request = match cli::parse(&args, |k| std::env::var(k).ok()) {
        Ok(Invocation::Help) => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Ok(Invocation::Run(r)) => r,
        Err(e) => {
            eprintln!("error: {e:#}\n\nRun `frosted-verify --help` for usage.");
            return ExitCode::from(2);
        }
    };
    match cli::execute(&request).await {
        Ok((text, ok)) => {
            println!("{}", text.trim_end());
            if ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}
