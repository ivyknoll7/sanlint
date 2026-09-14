//! Validation of a single SAN (Standard Algebraic Notation) move token.
//!
//! This checks syntax only: it has no board, so it cannot know whether a
//! given move is legal or whether "x" is warranted. What it can and does
//! check is everything the grammar itself fixes: piece letters, square
//! names, disambiguation shape, promotion rules, and the pawn-must-promote
//! rule, which falls out of knowing a pawn's destination rank.

use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum SanError {
    Empty,
    InvalidPiece(char),
    InvalidCastling(String),
    InvalidDestination(String),
    InvalidDisambiguation(String),
    MissingCaptureFile(String),
    MissingPromotion(String),
    UnexpectedPromotion(String),
    InvalidPromotion(String),
    MultipleCheckSymbols,
}

impl fmt::Display for SanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SanError::Empty => write!(f, "empty move"),
            SanError::InvalidPiece(c) => write!(f, "'{c}' is not a valid piece letter"),
            SanError::InvalidCastling(s) => write!(f, "'{s}' is not valid castling notation"),
            SanError::InvalidDestination(s) => write!(f, "'{s}' is not a valid destination square"),
            SanError::InvalidDisambiguation(s) => {
                write!(f, "'{s}' is not a valid disambiguation")
            }
            SanError::MissingCaptureFile(s) => {
                write!(f, "pawn capture '{s}' is missing its origin file")
            }
            SanError::MissingPromotion(s) => {
                write!(f, "pawn move '{s}' reaches the last rank but has no promotion")
            }
            SanError::UnexpectedPromotion(s) => {
                write!(f, "'{s}' is not a legal place for a promotion")
            }
            SanError::InvalidPromotion(s) => write!(f, "'{s}' is not a valid promotion piece"),
            SanError::MultipleCheckSymbols => write!(f, "more than one check/mate symbol"),
        }
    }
}

impl std::error::Error for SanError {}

/// Validate a single move token (no move number, no surrounding whitespace).
///
/// In strict mode this enforces standard SAN exactly: uppercase piece
/// letters, `O-O`/`O-O-O` castling, a mandatory origin file on pawn
/// captures, and uppercase `QRBN` promotions.
///
/// In lenient mode it additionally accepts: `0-0`/`0-0-0` castling written
/// with digit zero, a lowercase piece or promotion letter, a `:` in place
/// of `x` for captures, and a trailing `e.p.`/`ep` marker on en passant
/// captures.
pub fn validate(mv: &str, lenient: bool) -> Result<(), SanError> {
    if mv.is_empty() {
        return Err(SanError::Empty);
    }

    let mut chars: Vec<char> = mv.chars().collect();

    // Strip at most one trailing check/mate symbol; a second one left over
    // afterwards means the move had two.
    if matches!(chars.last(), Some('+') | Some('#')) {
        chars.pop();
        if matches!(chars.last(), Some('+') | Some('#')) {
            return Err(SanError::MultipleCheckSymbols);
        }
    }

    let core: String = chars.into_iter().collect();

    if core == "O-O" || core == "O-O-O" {
        return Ok(());
    }
    if lenient && (core == "0-0" || core == "0-0-0") {
        return Ok(());
    }
    if core.contains('O') || core.contains('-') {
        return Err(SanError::InvalidCastling(mv.to_string()));
    }

    let core_chars: Vec<char> = core.chars().collect();
    if core_chars.is_empty() {
        return Err(SanError::Empty);
    }

    let mut idx = 0;
    let mut piece: Option<char> = None;
    match core_chars[0] {
        'K' | 'Q' | 'R' | 'B' | 'N' => {
            piece = Some(core_chars[0]);
            idx = 1;
        }
        'P' if lenient => {
            idx = 1;
        }
        'P' => return Err(SanError::InvalidPiece('P')),
        c if lenient && "kqrbn".contains(c) => {
            piece = Some(c.to_ascii_uppercase());
            idx = 1;
        }
        _ => {}
    }

    let mut rest: String = core_chars[idx..].iter().collect();

    if lenient {
        let lower = rest.to_lowercase();
        if lower.ends_with("e.p.") {
            let new_len = rest.len() - 4;
            rest.truncate(new_len);
        } else if lower.ends_with("ep") && !lower.ends_with("=ep") {
            let new_len = rest.len() - 2;
            rest.truncate(new_len);
        }
    }

    let (before_promo, promo): (String, Option<String>) = match rest.find('=') {
        Some(pos) => {
            let tail = rest[pos + 1..].to_string();
            let head = rest[..pos].to_string();
            (head, Some(tail))
        }
        None => (rest.clone(), None),
    };

    let bp_chars: Vec<char> = before_promo.chars().collect();
    if bp_chars.len() < 2 {
        return Err(SanError::InvalidDestination(mv.to_string()));
    }

    let dest_file = bp_chars[bp_chars.len() - 2];
    let dest_rank = bp_chars[bp_chars.len() - 1];
    if !('a'..='h').contains(&dest_file) || !('1'..='8').contains(&dest_rank) {
        return Err(SanError::InvalidDestination(before_promo));
    }

    let mut prefix: Vec<char> = bp_chars[..bp_chars.len() - 2].to_vec();
    let mut capture = false;
    if let Some(&last) = prefix.last() {
        if last == 'x' || (lenient && last == ':') {
            capture = true;
            prefix.pop();
        }
    }
    let disamb: String = prefix.into_iter().collect();

    if piece.is_none() {
        if capture {
            let disamb_chars: Vec<char> = disamb.chars().collect();
            if disamb_chars.len() != 1 || !('a'..='h').contains(&disamb_chars[0]) {
                return Err(SanError::MissingCaptureFile(mv.to_string()));
            }
        } else if !disamb.is_empty() {
            return Err(SanError::InvalidDisambiguation(disamb));
        }
    } else {
        let disamb_chars: Vec<char> = disamb.chars().collect();
        match disamb_chars.len() {
            0 => {}
            1 => {
                let c = disamb_chars[0];
                if !('a'..='h').contains(&c) && !('1'..='8').contains(&c) {
                    return Err(SanError::InvalidDisambiguation(disamb));
                }
            }
            2 => {
                if !('a'..='h').contains(&disamb_chars[0]) || !('1'..='8').contains(&disamb_chars[1]) {
                    return Err(SanError::InvalidDisambiguation(disamb));
                }
            }
            _ => return Err(SanError::InvalidDisambiguation(disamb)),
        }
    }

    let on_last_rank = dest_rank == '1' || dest_rank == '8';
    match (piece, promo, on_last_rank) {
        (None, Some(p), true) => {
            let pc: Vec<char> = p.chars().collect();
            let valid = pc.len() == 1
                && (matches!(pc[0], 'Q' | 'R' | 'B' | 'N')
                    || (lenient && matches!(pc[0], 'q' | 'r' | 'b' | 'n')));
            if !valid {
                return Err(SanError::InvalidPromotion(p));
            }
        }
        (None, None, true) => return Err(SanError::MissingPromotion(mv.to_string())),
        (None, Some(p), false) => return Err(SanError::UnexpectedPromotion(p)),
        (Some(_), Some(p), _) => return Err(SanError::UnexpectedPromotion(p)),
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_moves() {
        for mv in ["e4", "Nf3", "Bb5", "O-O", "O-O-O", "exd5", "Qh4xe1", "Rfd1", "e8=Q", "Qxe8#"] {
            assert_eq!(validate(mv, false), Ok(()), "expected {mv} to be valid");
        }
    }

    #[test]
    fn rejects_lowercase_piece_when_strict() {
        assert!(validate("nf3", false).is_err());
        assert!(validate("nf3", true).is_ok());
    }

    #[test]
    fn rejects_pawn_capture_without_origin_file() {
        assert_eq!(
            validate("xd5", false),
            Err(SanError::MissingCaptureFile("xd5".to_string()))
        );
    }

    #[test]
    fn requires_promotion_on_last_rank() {
        assert_eq!(
            validate("e8", false),
            Err(SanError::MissingPromotion("e8".to_string()))
        );
        assert_eq!(validate("e8=Q", false), Ok(()));
    }

    #[test]
    fn zero_castling_only_lenient() {
        assert!(validate("0-0", false).is_err());
        assert!(validate("0-0", true).is_ok());
    }

    #[test]
    fn rejects_double_check_symbol() {
        assert_eq!(validate("Qxe8+#", false), Err(SanError::MultipleCheckSymbols));
    }
}
