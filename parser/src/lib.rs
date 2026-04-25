use std::iter::Peekable;

use ast::{ComponentDefinition, Expression, Main, Parameter, Port, Statement, Type, Varible};

mod tokenizer;

use tokenizer::tokenize;

use crate::tokenizer::{BracketShape, Keyword, Punctuation, Token};

trait Parse {
    fn parse(it: &mut Peekable<impl Iterator<Item = Token>>) -> Option<Self>
    where
        Self: Sized;
}

impl Parse for Type {
    fn parse(it: &mut Peekable<impl Iterator<Item = Token>>) -> Option<Self> {
        let type_name = match it.next()? {
            Token::Identifier(s) => s,
            _ => panic!("Expected ident"),
        };

        let params = it.next_if_map(|x| match x {
            Token::Block {
                shape: BracketShape::Angle,
                children: tokens,
            } => Ok(tokens),
            _ => Err(x),
        })?;

        todo!()
    }
}

impl Parse for Parameter {
    fn parse(it: &mut Peekable<impl Iterator<Item = Token>>) -> Option<Self> {
        let name = match it.next() {
            Some(Token::Identifier(s)) => s,
            _ => panic!("Expected Identifier"),
        };

        match it.next() {
            Some(Token::Punctuation(Punctuation::SemiColon)) => {}
            _ => panic!("Expected Colon"),
        };

        let t = Type::parse(it)?;

        Some(Parameter { name, t })
    }
}

impl Parse for ComponentDefinition {
    fn parse(it: &mut Peekable<impl Iterator<Item = Token>>) -> Option<Self>
    where
        Self: Sized,
    {
        let name = match it.next() {
            Some(Token::Identifier(s)) => s,
            _ => panic!("Not given var name"),
        };

        let parameters = std::iter::from_fn(|| Parameter::parse(it)).collect();

        let statements = std::iter::from_fn(|| Statement::parse(it)).collect();

        Self {
            name,
            parameters,
            statements,
        }
        .into()
    }
}

impl Parse for Varible {
    fn parse(it: &mut Peekable<impl Iterator<Item = Token>>) -> Option<Self>
    where
        Self: Sized,
    {
        let name = match it.next() {
            Some(Token::Identifier(s)) => s,
            _ => panic!("Not given var name"),
        };

        match it.next() {
            Some(Token::Punctuation(Punctuation::Equals)) => {}
            _ => panic!("Expected Equals"),
        }

        let expression = Expression::parse(it)?;

        Self {
            name,
            value: expression,
        }
        .into()
    }
}

impl Parse for Main {
    fn parse(it: &mut Peekable<impl Iterator<Item = Token>>) -> Option<Self> {
        match it.next() {
            Some(Token::Keyword(Keyword::Component)) => {
                Main::ComponentDefinition(ComponentDefinition::parse(it)?)
            }
            Some(Token::Keyword(Keyword::Constant)) => Main::Varible(Varible::parse(it)?),

            _ => panic!("Operation is a bad boy"),
        }
        .into()
    }
}

impl Parse for Port {
    fn parse(it: &mut Peekable<impl Iterator<Item = Token>>) -> Option<Self>
    where
        Self: Sized,
    {
        let name = match it.next() {
            Some(Token::Identifier(name)) => name,
            _ => panic!("bad bad bad"),
        };

        match it.next() {
            Some(Token::Punctuation(Punctuation::SemiColon)) => {}
            _ => panic!("Expected ;"),
        }

        Self { name }.into()
    }
}

impl Parse for Statement {
    fn parse(it: &mut Peekable<impl Iterator<Item = Token>>) -> Option<Self> {
        match it.next() {
            Some(Token::Keyword(Keyword::Let)) => Statement::VaribleDefinition(Varible::parse(it)?),
            Some(Token::Keyword(Keyword::Port)) => Statement::PortDefinition(Port::parse(it)?),
            _ => panic!("invalid statement"),
        }
        .into()
    }
}

impl Parse for Expression {
    fn parse(it: &mut Peekable<impl Iterator<Item = Token>>) -> Option<Self> {
        todo!()
    }
}

fn parse(file: &str) -> Vec<Main> {
    println!("running");
    let tokens = tokenize(file);

    let mut it = tokens.into_iter().peekable();

    let main = std::iter::from_fn(move || Main::parse(&mut it));

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
