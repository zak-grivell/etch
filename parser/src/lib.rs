use ast::Main;

mod splitter;
mod tokenizer;

use tokenizer::tokenize;
// each {} signifies

fn parse(file: &str) -> Vec<Main> {
    println!("running");
    let tokens = tokenize(file);

    dbg!(tokens);

    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        parse(
            "
#this is a comment
component VoltageDivider(r1: Resistance, r2: Resistance) {
	port vcc: Analog;
	port mid: Analog;
	port gnd: Analog;

	let resistor_one = Resistor<a:vcc, b:mid>(value: r1);
	let resistor_two = Resistor<a:mid, b:gnd>(value: r2);
}
",
        );
    }
}
