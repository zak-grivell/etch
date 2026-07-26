use std::{fs::read_to_string, path::PathBuf};

use clap::Parser;
use parser::{compile, print_errors};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    file_path: PathBuf,
}
fn main() {
    let args = Args::parse();

    let binding = args.file_path.clone();
    let filename = binding
        .file_name()
        .expect("could not get filename")
        .to_string_lossy();

    let input = read_to_string(args.file_path).expect("could not find file");

    let output = match compile(&filename, &input) {
        Ok(result) => result,
        Err(errors) => {
            print_errors(&filename, &input, errors);
            return;
        }
    };

    println!("{:?}", output)
}
