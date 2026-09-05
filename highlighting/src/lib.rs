use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HighlightKind {
    Comment,
    Keyword,
    String,
    Number,
    Boolean,
    Type,
    Identifier,
    Property,
    Operator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Highlight {
    pub span: Range<usize>,
    pub kind: HighlightKind,
}

pub fn highlight(source: &str) -> Vec<Highlight> {
    let mut result = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        let rest = &source[cursor..];
        let ch = rest.chars().next().unwrap();
        if ch.is_whitespace() {
            cursor += ch.len_utf8();
        } else if rest.starts_with("//") {
            let end = rest
                .find('\n')
                .map_or(source.len(), |offset| cursor + offset);
            result.push(token(cursor..end, HighlightKind::Comment));
            cursor = end;
        } else if ch == '"' {
            let mut end = cursor + 1;
            let mut escaped = false;
            for next in source[end..].chars() {
                end += next.len_utf8();
                if next == '"' && !escaped {
                    break;
                }
                escaped = next == '\\' && !escaped;
                if next != '\\' {
                    escaped = false;
                }
            }
            result.push(token(cursor..end, HighlightKind::String));
            cursor = end;
        } else if ch.is_ascii_digit() {
            let end = take_while(source, cursor, |c| {
                c.is_ascii_alphanumeric() || c == '.' || !c.is_ascii()
            });
            result.push(token(cursor..end, HighlightKind::Number));
            cursor = end;
        } else if ch.is_ascii_alphabetic() || ch == '_' {
            let end = take_while(source, cursor, |c| c.is_ascii_alphanumeric() || c == '_');
            let word = &source[cursor..end];
            let kind = if matches!(
                word,
                "let" | "return" | "match" | "type" | "from" | "import" | "export" | "if"
            ) {
                HighlightKind::Keyword
            } else if matches!(word, "true" | "false") {
                HighlightKind::Boolean
            } else if word.chars().next().is_some_and(char::is_uppercase) {
                HighlightKind::Type
            } else if source[end..].trim_start().starts_with(':') {
                HighlightKind::Property
            } else {
                HighlightKind::Identifier
            };
            result.push(token(cursor..end, kind));
            cursor = end;
        } else {
            let width = if ["==", "<=", ">=", "<-", "->"]
                .iter()
                .any(|operator| rest.starts_with(operator))
            {
                2
            } else {
                ch.len_utf8()
            };
            let end = cursor + width;
            result.push(token(cursor..end, HighlightKind::Operator));
            cursor = end;
        }
    }
    result
}

fn take_while(source: &str, start: usize, predicate: impl Fn(char) -> bool) -> usize {
    let mut end = start;
    for ch in source[start..].chars() {
        if !predicate(ch) {
            break;
        }
        end += ch.len_utf8();
    }
    end
}

fn token(span: Range<usize>, kind: HighlightKind) -> Highlight {
    Highlight { span, kind }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_incomplete_source_without_failing() {
        let tokens = highlight("let voltage: Voltage = 5V; // supply\n\"unfinished");
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == HighlightKind::Keyword)
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == HighlightKind::Comment)
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == HighlightKind::String)
        );
        assert!(tokens.iter().all(|token| token.span.end <= 51));
    }
}

#[cfg(test)]
#[test]
fn recognizes_only_supported_compound_operators() {
    let source = "<- -> <= >= == =>";
    let operators: Vec<_> = highlight(source)
        .into_iter()
        .map(|token| &source[token.span])
        .collect();
    assert_eq!(operators, ["<-", "->", "<=", ">=", "==", "=", ">"]);
}

#[cfg(test)]
#[test]
fn escaped_quotes_remain_inside_one_string_token() {
    let source = r#""a\"b\\c\n"; next"#;
    let tokens = highlight(source);
    assert_eq!(tokens[0].kind, HighlightKind::String);
    assert_eq!(&source[tokens[0].span.clone()], r#""a\"b\\c\n""#);
    assert_eq!(&source[tokens.last().unwrap().span.clone()], "next");
}
