use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    match cli::execute(cli::Cli::parse()) {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
