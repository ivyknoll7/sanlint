//! Minimal PGN movetext tokenizer.
//!
//! Enough to pull move tokens out of real PGN so `sanlint` can be pointed
//! at an actual game file: it drops header lines (`[Event "..."]`), brace
//! comments, semicolon comments, and parenthesized variations, and tracks
//! line numbers so errors can point somewhere useful.
//!
//! Headers are assumed to be one per line, which holds for every PGN
//! writer in practice even though the spec allows more.

pub struct Token {
    pub line: u32,
    pub text: String,
}

pub fn tokenize(input: &str) -> Vec<Token> {
    fn flush(word: &mut String, tokens: &mut Vec<Token>, line: u32) {
        if !word.is_empty() {
            tokens.push(Token {
                line,
                text: std::mem::take(word),
            });
        }
    }

    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut line: u32 = 1;
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        match c {
            '\n' => {
                flush(&mut word, &mut tokens, line);
                line += 1;
                i += 1;
            }
            '[' if word.is_empty() => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '{' => {
                flush(&mut word, &mut tokens, line);
                i += 1;
                while i < chars.len() && chars[i] != '}' {
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    i += 1;
                }
                i += 1;
            }
            ';' => {
                flush(&mut word, &mut tokens, line);
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '(' => {
                flush(&mut word, &mut tokens, line);
                let mut depth = 1;
                i += 1;
                while i < chars.len() && depth > 0 {
                    match chars[i] {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        '\n' => line += 1,
                        _ => {}
                    }
                    i += 1;
                }
            }
            c if c.is_whitespace() => {
                flush(&mut word, &mut tokens, line);
                i += 1;
            }
            _ => {
                word.push(c);
                i += 1;
            }
        }
    }
    flush(&mut word, &mut tokens, line);
    tokens
}

/// Strip a leading move-number marker such as "12." or "12..." from a
/// token, leaving the move itself. Tokens that are not a move number
/// (including plain moves, which never start with a digit) pass through
/// unchanged.
pub fn strip_move_number(word: &str) -> &str {
    let bytes = word.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return word;
    }
    let mut j = i;
    while j < bytes.len() && bytes[j] == b'.' {
        j += 1;
    }
    if j == i {
        // digits with no following dot: not a move-number marker
        return word;
    }
    &word[j..]
}

pub fn is_result(word: &str) -> bool {
    matches!(word, "1-0" | "0-1" | "1/2-1/2" | "*")
}

pub fn is_nag(word: &str) -> bool {
    let bytes = word.as_bytes();
    bytes.len() > 1 && bytes[0] == b'$' && bytes[1..].iter().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_headers_and_comments() {
        let pgn = "[Event \"Test\"]\n[Site \"Nowhere\"]\n\n1. e4 {a comment} e5 2. Nf3 Nc6 *";
        let words: Vec<String> = tokenize(pgn).into_iter().map(|t| t.text).collect();
        assert_eq!(words, vec!["1.", "e4", "e5", "2.", "Nf3", "Nc6", "*"]);
    }

    #[test]
    fn skips_variations() {
        let pgn = "1. e4 (1. d4 d5) e5";
        let words: Vec<String> = tokenize(pgn).into_iter().map(|t| t.text).collect();
        assert_eq!(words, vec!["1.", "e4", "e5"]);
    }

    #[test]
    fn strips_move_numbers() {
        assert_eq!(strip_move_number("12."), "");
        assert_eq!(strip_move_number("12...e4"), "e4");
        assert_eq!(strip_move_number("e4"), "e4");
    }
}
