//! Validation and structural parsing of a single SAN (Standard Algebraic
//! Notation) move token.
//!
//! `validate` checks syntax only: piece letters, square names,
//! disambiguation shape, promotion rules, and the pawn-must-promote rule.
//! `parse` does the same grammar work but returns the pieces it found
//! (which piece, which square, whether it was a capture, ...) so a caller
//! that tracks a board, like `board::Board`, doesn't have to re-derive them
//! from the raw string.

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Piece {
    King,
    Queen,
    Rook,
    Bishop,
    Knight,
    Pawn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Castle {
    Kingside,
    Queenside,
}

/// The structural content of a syntactically valid move: which piece moved,
/// any disambiguating file/rank, whether it captured, where it landed, and
/// what it promoted to. `dest` is `None` only for castling, where the
/// landing square depends on which side is moving rather than on the move
/// text itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub castle: Option<Castle>,
    pub piece: Piece,
    pub disambig_file: Option<char>,
    pub disambig_rank: Option<char>,
    pub capture: bool,
    pub dest: Option<(char, char)>,
    pub promotion: Option<Piece>,
}

/// Validate a single move token (no move number, no surrounding whitespace).
///
/// In strict mode this enforces standard SAN exactly: uppercase piece
/// letters, `O-O`/`O-O-O` castling, a mandatory origin file on pawn
/// captures, and uppercase `QRBN` promotions.
///
/// In lenient mode it additionally accepts: `0-0`/`0-0-0` castling written
/// with digit zero, a lowercase piece or promotion letter, a `:` in place
/// of `x` for captures, and a trailing `e.p.`/`ep` marker on an en passant
/// capture.
pub fn validate(mv: &str, lenient: bool) -> Result<(), SanError> {
    parse(mv, lenient).map(|_| ())
}

/// Same grammar as `validate`, but returns the parsed move on success
/// instead of discarding it.
pub fn parse(mv: &str, lenient: bool) -> Result<Move, SanError> {
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

    let castle_move = |castle: Castle| Move {
        castle: Some(castle),
        piece: Piece::King,
        disambig_file: None,
        disambig_rank: None,
        capture: false,
        dest: None,
        promotion: None,
    };

    if core == "O-O" {
        return Ok(castle_move(Castle::Kingside));
    }
    if core == "O-O-O" {
        return Ok(castle_move(Castle::Queenside));
    }
    if lenient && core == "0-0" {
        return Ok(castle_move(Castle::Kingside));
    }
    if lenient && core == "0-0-0" {
        return Ok(castle_move(Castle::Queenside));
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

    let mut disambig_file: Option<char> = None;
    let mut disambig_rank: Option<char> = None;

    if piece.is_none() {
        if capture {
            let disamb_chars: Vec<char> = disamb.chars().collect();
            if disamb_chars.len() != 1 || !('a'..='h').contains(&disamb_chars[0]) {
                return Err(SanError::MissingCaptureFile(mv.to_string()));
            }
            disambig_file = Some(disamb_chars[0]);
        } else if !disamb.is_empty() {
            return Err(SanError::InvalidDisambiguation(disamb));
        }
    } else {
        let disamb_chars: Vec<char> = disamb.chars().collect();
        match disamb_chars.len() {
            0 => {}
            1 => {
                let c = disamb_chars[0];
                if ('a'..='h').contains(&c) {
                    disambig_file = Some(c);
                } else if ('1'..='8').contains(&c) {
                    disambig_rank = Some(c);
                } else {
                    return Err(SanError::InvalidDisambiguation(disamb));
                }
            }
            2 => {
                if !('a'..='h').contains(&disamb_chars[0]) || !('1'..='8').contains(&disamb_chars[1]) {
                    return Err(SanError::InvalidDisambiguation(disamb));
                }
                disambig_file = Some(disamb_chars[0]);
                disambig_rank = Some(disamb_chars[1]);
            }
            _ => return Err(SanError::InvalidDisambiguation(disamb)),
        }
    }

    let on_last_rank = dest_rank == '1' || dest_rank == '8';
    let mut promotion: Option<Piece> = None;
    match (piece, promo.as_deref(), on_last_rank) {
        (None, Some(p), true) => {
            let pc: Vec<char> = p.chars().collect();
            let valid = pc.len() == 1
                && (matches!(pc[0], 'Q' | 'R' | 'B' | 'N')
                    || (lenient && matches!(pc[0], 'q' | 'r' | 'b' | 'n')));
            if !valid {
                return Err(SanError::InvalidPromotion(p.to_string()));
            }
            promotion = Some(piece_from_char(pc[0].to_ascii_uppercase()));
        }
        (None, None, true) => return Err(SanError::MissingPromotion(mv.to_string())),
        (None, Some(p), false) => return Err(SanError::UnexpectedPromotion(p.to_string())),
        (Some(_), Some(p), _) => return Err(SanError::UnexpectedPromotion(p.to_string())),
        _ => {}
    }

    let piece_kind = match piece {
        Some(c) => piece_from_char(c),
        None => Piece::Pawn,
    };

    Ok(Move {
        castle: None,
        piece: piece_kind,
        disambig_file,
        disambig_rank,
        capture,
        dest: Some((dest_file, dest_rank)),
        promotion,
    })
}

fn piece_from_char(c: char) -> Piece {
    match c {
        'K' => Piece::King,
        'Q' => Piece::Queen,
        'R' => Piece::Rook,
        'B' => Piece::Bishop,
        'N' => Piece::Knight,
        _ => unreachable!("piece_from_char called with '{c}', which is not a piece letter"),
    }
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

    #[test]
    fn parse_reports_move_shape() {
        let mv = parse("Nbd7", false).unwrap();
        assert_eq!(mv.piece, Piece::Knight);
        assert_eq!(mv.disambig_file, Some('b'));
        assert_eq!(mv.disambig_rank, None);
        assert!(!mv.capture);
        assert_eq!(mv.dest, Some(('d', '7')));
    }

    #[test]
    fn parse_reports_capture_and_promotion() {
        let mv = parse("exd8=Q", false).unwrap();
        assert_eq!(mv.piece, Piece::Pawn);
        assert_eq!(mv.disambig_file, Some('e'));
        assert!(mv.capture);
        assert_eq!(mv.dest, Some(('d', '8')));
        assert_eq!(mv.promotion, Some(Piece::Queen));
    }

    #[test]
    fn parse_reports_castling_with_no_destination() {
        let mv = parse("O-O", false).unwrap();
        assert_eq!(mv.castle, Some(Castle::Kingside));
        assert_eq!(mv.dest, None);
    }
}
