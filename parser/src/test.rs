use std::fs::read_to_string;

use crate::{compile, print_errors};

#[test]
pub fn test() {
    let filename = "main.etch";

    let input = read_to_string("../examples/main.etch").unwrap();

    let compile_result = compile(&input);

    match compile_result {
        Ok(result) => println!("{:?}", result),
        Err(compiler_errors) => {
            print_errors(filename, &input, compiler_errors);
        }
    }
}
