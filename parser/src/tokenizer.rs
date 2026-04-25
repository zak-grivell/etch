use std::{
    iter::{self, Peekable},
    str::Chars,
};

use itertools::{Itertools, PeekingNext};

#[derive(Debug, Clone, Copy)]
pub enum Punctuation {
    SemiColon,
    Colon,
    Equals,
    Comma,
}

#[derive(Debug, Clone, Copy)]
pub enum BracketShape {
    Parentathis,
    Squiggle,
    Square,
    Angle,
}

impl BracketShape {
    fn opening(&self) -> char {
        match self {
            BracketShape::Parentathis => '(',
            BracketShape::Squiggle => '{',
            BracketShape::Square => '[',
            BracketShape::Angle => '<',
        }
    }

    fn closing(&self) -> char {
        match self {
            BracketShape::Parentathis => ')',
            BracketShape::Squiggle => '}',
            BracketShape::Square => ']',
            BracketShape::Angle => '>',
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum BracketState {
    Open,
    Close,
}

struct Bracket {
    shape: BracketShape,
    state: BracketState,
}

impl Bracket {
    fn parse(value: char) -> Option<Self> {
        match value {
            '{' => Some(Self {
                shape: BracketShape::Squiggle,
                state: BracketState::Open,
            }),
            '}' => Some(Self {
                shape: BracketShape::Squiggle,
                state: BracketState::Close,
            }),
            '<' => Some(Self {
                shape: BracketShape::Angle,
                state: BracketState::Open,
            }),
            '>' => Some(Self {
                shape: BracketShape::Angle,
                state: BracketState::Close,
            }),
            '[' => Some(Self {
                shape: BracketShape::Square,
                state: BracketState::Open,
            }),
            ']' => Some(Self {
                shape: BracketShape::Square,
                state: BracketState::Close,
            }),
            '(' => Some(Self {
                shape: BracketShape::Parentathis,
                state: BracketState::Open,
            }),
            ')' => Some(Self {
                shape: BracketShape::Parentathis,
                state: BracketState::Close,
            }),
            _ => None,
        }
    }
}

impl Punctuation {
    fn parse(value: char) -> Option<Self> {
        match value {
            ';' => Some(Punctuation::SemiColon),
            ':' => Some(Punctuation::Colon),
            ',' => Some(Punctuation::Comma),
            '=' => Some(Punctuation::Equals),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum Keyword {
    Component,
    Constant,
    Port,
    Let,
    For,
    Match,
}

impl Keyword {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "component" => Some(Self::Component),
            "port" => Some(Self::Port),
            "let" => Some(Self::Let),
            "for" => Some(Self::For),
            "match" => Some(Self::Match),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum Token {
    Punctuation(Punctuation),
    Number {
        num: i32,
        suffix: String,
    },
    String(String),
    Keyword(Keyword),
    Identifier(String),
    Block {
        shape: BracketShape,
        children: Vec<Token>,
    },
}

pub fn tokenize<'a>(file: &'a str) -> Vec<Token> {
    let mut items = file.chars().peekable();

    iter::from_fn(move || tokenize_step(&mut items)).collect()
}

pub fn tokenize_step(file: &mut Peekable<Chars>) -> Option<Token> {
    let character = *file.peek()?;

    if character.is_whitespace() {
        file.next();

        return tokenize_step(file);
    }

    if character == '#' {
        file.take_while(|&s| s != '\n').for_each(|_| {});

        return tokenize_step(file);
    }

    if let Some(bracket) = Bracket::parse(character) {
        file.next();

        let mut i = 1;
        let end = bracket.shape.closing();
        let start = bracket.shape.opening();

        let inner = file
            .by_ref()
            .take_while(|&c| {
                if c == start {
                    i += 1
                } else if c == end {
                    i -= 1
                }

                i != 0
            })
            .collect::<String>();

        return Token::Block {
            shape: bracket.shape,
            children: tokenize(&inner),
        }
        .into();
    }

    if character.is_ascii_digit() {
        return Token::Number {
            num: file
                .peeking_take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap(),
            suffix: file
                .peeking_take_while(|c| c.is_ascii_alphabetic())
                .collect(),
        }
        .into();
    }

    dbg!(character, Punctuation::parse(character));

    if let Some(punctuation) = Punctuation::parse(character) {
        file.next();

        return Token::Punctuation(punctuation).into();
    }

    if character.is_ascii_alphabetic() {
        let word = file
            .peeking_take_while(|&c| c.is_ascii_alphanumeric() || c == '_')
            .collect::<String>();

        return if let Some(keyword) = Keyword::parse(word.as_str()) {
            Token::Keyword(keyword)
        } else {
            Token::Identifier(word)
        }
        .into();
    }

    if character == '"' {
        file.next();
        let string = file.by_ref().peeking_take_while(|&s| s != '"').collect();
        file.next();

        return Token::String(string).into();
    }

    panic!("bad char {}", character);
}
