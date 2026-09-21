//! A minimal board model used to check whether a syntactically valid move
//! actually has a piece that can make it.
//!
//! This tracks piece placement, side to move, castling rights, and the en
//! passant square through a sequence of moves, starting from a FEN
//! position (the standard start position by default). For each move it
//! finds which piece of the right kind, on the right side, matching any
//! disambiguator, can reach the destination square given where pieces
//! currently sit on the board. It rejects moves with no such piece, or
//! more than one.
//!
//! What it does not do: verify that a move doesn't leave the mover's own
//! king in check. Pin and check detection require generating the
//! opponent's replies too, which this board does not do yet. So a move
//! that is otherwise well-formed and geometrically possible is accepted
//! even if it would illegally expose the king.

use crate::san;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
}

impl Color {
    fn other(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Occupant {
    color: Color,
    piece: san::Piece,
}

struct CastleRights {
    white_k: bool,
    white_q: bool,
    black_k: bool,
    black_q: bool,
}

pub struct Board {
    squares: [[Option<Occupant>; 8]; 8],
    side_to_move: Color,
    castling: CastleRights,
    en_passant: Option<(u8, u8)>,
}

const STARTING_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

impl Board {
    pub fn start_position() -> Board {
        Board::from_fen(STARTING_FEN).expect("built-in starting FEN is well-formed")
    }

    pub fn from_fen(fen: &str) -> Result<Board, String> {
        let mut fields = fen.split_whitespace();
        let placement = fields.next().ok_or("FEN is missing a piece placement field")?;
        let side = fields.next().unwrap_or("w");
        let castling = fields.next().unwrap_or("-");
        let ep = fields.next().unwrap_or("-");

        let mut squares: [[Option<Occupant>; 8]; 8] = [[None; 8]; 8];
        let ranks: Vec<&str> = placement.split('/').collect();
        if ranks.len() != 8 {
            return Err(format!("FEN placement has {} ranks, expected 8", ranks.len()));
        }
        for (i, rank_str) in ranks.iter().enumerate() {
            let rank = 7 - i as u8;
            let mut file: u8 = 0;
            for c in rank_str.chars() {
                if let Some(skip) = c.to_digit(10) {
                    file += skip as u8;
                    continue;
                }
                if file >= 8 {
                    return Err(format!("FEN rank '{rank_str}' has too many squares"));
                }
                let color = if c.is_uppercase() { Color::White } else { Color::Black };
                let piece = match c.to_ascii_uppercase() {
                    'K' => san::Piece::King,
                    'Q' => san::Piece::Queen,
                    'R' => san::Piece::Rook,
                    'B' => san::Piece::Bishop,
                    'N' => san::Piece::Knight,
                    'P' => san::Piece::Pawn,
                    other => return Err(format!("'{other}' is not a valid FEN piece letter")),
                };
                squares[file as usize][rank as usize] = Some(Occupant { color, piece });
                file += 1;
            }
            if file != 8 {
                return Err(format!("FEN rank '{rank_str}' does not cover all 8 files"));
            }
        }

        let side_to_move = match side {
            "w" => Color::White,
            "b" => Color::Black,
            other => return Err(format!("'{other}' is not a valid FEN side to move")),
        };

        let castling = CastleRights {
            white_k: castling.contains('K'),
            white_q: castling.contains('Q'),
            black_k: castling.contains('k'),
            black_q: castling.contains('q'),
        };

        let en_passant = if ep == "-" {
            None
        } else {
            match square_from_str(ep) {
                Some(sq) => Some(sq),
                None => return Err(format!("'{ep}' is not a valid FEN en passant square")),
            }
        };

        Ok(Board { squares, side_to_move, castling, en_passant })
    }

    pub fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    pub fn piece_at(&self, square: &str) -> Option<(Color, san::Piece)> {
        let (file, rank) = square_from_str(square)?;
        self.squares[file as usize][rank as usize].map(|o| (o.color, o.piece))
    }

    /// Apply a parsed move to the board. On success the board reflects the
    /// position after the move and it becomes the other side's turn. On
    /// failure the board is left unchanged.
    pub fn apply(&mut self, mv: &san::Move) -> Result<(), String> {
        if let Some(castle) = mv.castle {
            return self.apply_castle(castle);
        }

        let (dest_file_c, dest_rank_c) = match mv.dest {
            Some(d) => d,
            None => return Err("move has no destination square".to_string()),
        };
        let dest = (file_index(dest_file_c), rank_index(dest_rank_c));

        let mut candidates = Vec::new();
        for file in 0..8u8 {
            for rank in 0..8u8 {
                let occ = match self.squares[file as usize][rank as usize] {
                    Some(o) => o,
                    None => continue,
                };
                if occ.color != self.side_to_move || occ.piece != mv.piece {
                    continue;
                }
                if let Some(df) = mv.disambig_file {
                    if file != file_index(df) {
                        continue;
                    }
                }
                if let Some(dr) = mv.disambig_rank {
                    if rank != rank_index(dr) {
                        continue;
                    }
                }
                if self.can_reach((file, rank), dest, mv.piece, mv.capture) {
                    candidates.push((file, rank));
                }
            }
        }

        match candidates.len() {
            0 => Err(format!(
                "no {} can reach {}{} from the current position",
                piece_name(mv.piece),
                dest_file_c,
                dest_rank_c
            )),
            1 => self.make_move(candidates[0], dest, mv),
            n => Err(format!(
                "move to {}{} is ambiguous between {} pieces",
                dest_file_c, dest_rank_c, n
            )),
        }
    }

    fn can_reach(&self, from: (u8, u8), to: (u8, u8), piece: san::Piece, capture: bool) -> bool {
        let df = to.0 as i8 - from.0 as i8;
        let dr = to.1 as i8 - from.1 as i8;
        let dest_occupant = self.squares[to.0 as usize][to.1 as usize];

        match piece {
            san::Piece::Pawn => {
                let dir: i8 = if self.side_to_move == Color::White { 1 } else { -1 };
                let start_rank: i8 = if self.side_to_move == Color::White { 1 } else { 6 };
                if capture {
                    if df.abs() != 1 || dr != dir {
                        return false;
                    }
                    let is_enemy = dest_occupant.map_or(false, |o| o.color != self.side_to_move);
                    is_enemy || self.en_passant == Some(to)
                } else {
                    if df != 0 || dest_occupant.is_some() {
                        return false;
                    }
                    if dr == dir {
                        return true;
                    }
                    if from.1 as i8 == start_rank && dr == 2 * dir {
                        let mid_rank = (from.1 as i8 + dir) as u8;
                        return self.squares[from.0 as usize][mid_rank as usize].is_none();
                    }
                    false
                }
            }
            san::Piece::Knight => {
                (df.abs() == 1 && dr.abs() == 2 || df.abs() == 2 && dr.abs() == 1)
                    && dest_ok(dest_occupant, capture, self.side_to_move)
            }
            san::Piece::King => {
                df.abs() <= 1
                    && dr.abs() <= 1
                    && (df != 0 || dr != 0)
                    && dest_ok(dest_occupant, capture, self.side_to_move)
            }
            san::Piece::Bishop => {
                df.abs() == dr.abs()
                    && df != 0
                    && self.path_clear(from, to)
                    && dest_ok(dest_occupant, capture, self.side_to_move)
            }
            san::Piece::Rook => {
                (df == 0) != (dr == 0)
                    && self.path_clear(from, to)
                    && dest_ok(dest_occupant, capture, self.side_to_move)
            }
            san::Piece::Queen => {
                ((df == 0) != (dr == 0) || (df.abs() == dr.abs() && df != 0))
                    && self.path_clear(from, to)
                    && dest_ok(dest_occupant, capture, self.side_to_move)
            }
        }
    }

    fn path_clear(&self, from: (u8, u8), to: (u8, u8)) -> bool {
        let step_f = (to.0 as i8 - from.0 as i8).signum();
        let step_r = (to.1 as i8 - from.1 as i8).signum();
        let mut f = from.0 as i8 + step_f;
        let mut r = from.1 as i8 + step_r;
        while (f, r) != (to.0 as i8, to.1 as i8) {
            if self.squares[f as usize][r as usize].is_some() {
                return false;
            }
            f += step_f;
            r += step_r;
        }
        true
    }

    fn make_move(&mut self, from: (u8, u8), to: (u8, u8), mv: &san::Move) -> Result<(), String> {
        let moving = match self.squares[from.0 as usize][from.1 as usize] {
            Some(o) => o,
            None => return Err("internal error: candidate square has no piece".to_string()),
        };

        if moving.piece == san::Piece::Pawn
            && mv.capture
            && self.squares[to.0 as usize][to.1 as usize].is_none()
        {
            // En passant: the captured pawn sits beside the mover, on the
            // mover's own departure rank, not on the destination square.
            self.squares[to.0 as usize][from.1 as usize] = None;
        }

        self.squares[from.0 as usize][from.1 as usize] = None;

        let placed = match mv.promotion {
            Some(p) => Occupant { color: moving.color, piece: p },
            None => moving,
        };
        self.squares[to.0 as usize][to.1 as usize] = Some(placed);

        self.update_castling_rights(from, moving);
        self.update_castling_rights_on_capture(to);

        self.en_passant = if moving.piece == san::Piece::Pawn && (to.1 as i8 - from.1 as i8).abs() == 2 {
            Some((from.0, ((from.1 as i8 + to.1 as i8) / 2) as u8))
        } else {
            None
        };

        self.side_to_move = self.side_to_move.other();
        Ok(())
    }

    fn apply_castle(&mut self, castle: san::Castle) -> Result<(), String> {
        let (rank, allowed) = match (self.side_to_move, castle) {
            (Color::White, san::Castle::Kingside) => (0u8, self.castling.white_k),
            (Color::White, san::Castle::Queenside) => (0u8, self.castling.white_q),
            (Color::Black, san::Castle::Kingside) => (7u8, self.castling.black_k),
            (Color::Black, san::Castle::Queenside) => (7u8, self.castling.black_q),
        };

        if !allowed {
            return Err(format!("castling rights for {castle:?}-side castling are not available"));
        }

        let (king_from, king_to, rook_from, rook_to, path_files) = match castle {
            san::Castle::Kingside => ((4, rank), (6, rank), (7, rank), (5, rank), vec![5u8, 6]),
            san::Castle::Queenside => ((4, rank), (2, rank), (0, rank), (3, rank), vec![1u8, 2, 3]),
        };

        for f in path_files {
            if self.squares[f as usize][rank as usize].is_some() {
                return Err(format!("castling is blocked: {}{} is occupied", file_letter(f), rank + 1));
            }
        }

        let king = match self.squares[king_from.0 as usize][king_from.1 as usize] {
            Some(o) => o,
            None => return Err("no king on its home square to castle".to_string()),
        };
        let rook = match self.squares[rook_from.0 as usize][rook_from.1 as usize] {
            Some(o) => o,
            None => return Err("no rook on its home square to castle".to_string()),
        };

        self.squares[king_from.0 as usize][king_from.1 as usize] = None;
        self.squares[rook_from.0 as usize][rook_from.1 as usize] = None;
        self.squares[king_to.0 as usize][king_to.1 as usize] = Some(king);
        self.squares[rook_to.0 as usize][rook_to.1 as usize] = Some(rook);

        match self.side_to_move {
            Color::White => {
                self.castling.white_k = false;
                self.castling.white_q = false;
            }
            Color::Black => {
                self.castling.black_k = false;
                self.castling.black_q = false;
            }
        }

        self.en_passant = None;
        self.side_to_move = self.side_to_move.other();
        Ok(())
    }

    fn update_castling_rights(&mut self, from: (u8, u8), moving: Occupant) {
        match (moving.color, moving.piece) {
            (Color::White, san::Piece::King) => {
                self.castling.white_k = false;
                self.castling.white_q = false;
            }
            (Color::Black, san::Piece::King) => {
                self.castling.black_k = false;
                self.castling.black_q = false;
            }
            (Color::White, san::Piece::Rook) if from == (0, 0) => self.castling.white_q = false,
            (Color::White, san::Piece::Rook) if from == (7, 0) => self.castling.white_k = false,
            (Color::Black, san::Piece::Rook) if from == (0, 7) => self.castling.black_q = false,
            (Color::Black, san::Piece::Rook) if from == (7, 7) => self.castling.black_k = false,
            _ => {}
        }
    }

    fn update_castling_rights_on_capture(&mut self, at: (u8, u8)) {
        match at {
            (0, 0) => self.castling.white_q = false,
            (7, 0) => self.castling.white_k = false,
            (0, 7) => self.castling.black_q = false,
            (7, 7) => self.castling.black_k = false,
            _ => {}
        }
    }
}

fn dest_ok(dest_occupant: Option<Occupant>, capture: bool, side_to_move: Color) -> bool {
    match dest_occupant {
        None => !capture,
        Some(o) => capture && o.color != side_to_move,
    }
}

fn piece_name(p: san::Piece) -> &'static str {
    match p {
        san::Piece::King => "king",
        san::Piece::Queen => "queen",
        san::Piece::Rook => "rook",
        san::Piece::Bishop => "bishop",
        san::Piece::Knight => "knight",
        san::Piece::Pawn => "pawn",
    }
}

fn file_index(c: char) -> u8 {
    c as u8 - b'a'
}

fn rank_index(c: char) -> u8 {
    c as u8 - b'1'
}

fn file_letter(f: u8) -> char {
    (b'a' + f) as char
}

fn square_from_str(s: &str) -> Option<(u8, u8)> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() != 2 {
        return None;
    }
    let (file, rank) = (chars[0], chars[1]);
    if !('a'..='h').contains(&file) || !('1'..='8').contains(&rank) {
        return None;
    }
    Some((file_index(file), rank_index(rank)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply_moves(board: &mut Board, moves: &[&str]) {
        for mv in moves {
            let parsed = san::parse(mv, false).unwrap_or_else(|e| panic!("'{mv}' failed to parse: {e}"));
            board.apply(&parsed).unwrap_or_else(|e| panic!("'{mv}' rejected: {e}"));
        }
    }

    #[test]
    fn plays_out_a_ruy_lopez_opening() {
        let mut board = Board::start_position();
        apply_moves(&mut board, &["e4", "e5", "Nf3", "Nc6", "Bb5"]);
        assert_eq!(board.piece_at("b5"), Some((Color::White, san::Piece::Bishop)));
        assert_eq!(board.piece_at("e4"), Some((Color::White, san::Piece::Pawn)));
        assert_eq!(board.piece_at("f1"), None);
    }

    #[test]
    fn rejects_a_queen_move_blocked_by_its_own_pawn() {
        let mut board = Board::start_position();
        let mv = san::parse("Qh5", false).unwrap();
        assert!(board.apply(&mv).is_err());
    }

    #[test]
    fn rejects_a_move_with_no_matching_piece() {
        let mut board = Board::start_position();
        let mv = san::parse("Nd5", false).unwrap();
        assert!(board.apply(&mv).is_err());
    }

    #[test]
    fn castles_kingside_once_the_path_is_clear() {
        let mut board = Board::start_position();
        apply_moves(&mut board, &["e4", "e5", "Nf3", "Nc6", "Bc4", "Bc5", "O-O"]);
        assert_eq!(board.piece_at("g1"), Some((Color::White, san::Piece::King)));
        assert_eq!(board.piece_at("f1"), Some((Color::White, san::Piece::Rook)));
        assert_eq!(board.piece_at("e1"), None);
        assert_eq!(board.piece_at("h1"), None);
    }

    #[test]
    fn captures_en_passant() {
        let mut board = Board::start_position();
        apply_moves(&mut board, &["e4", "a6", "e5", "d5"]);
        assert_eq!(board.piece_at("d5"), Some((Color::Black, san::Piece::Pawn)));
        apply_moves(&mut board, &["exd6"]);
        assert_eq!(board.piece_at("d6"), Some((Color::White, san::Piece::Pawn)));
        assert_eq!(board.piece_at("d5"), None);
        assert_eq!(board.piece_at("e5"), None);
    }

    #[test]
    fn parses_fen_placement_and_side_to_move() {
        let board = Board::from_fen("8/8/8/8/8/8/4P3/4K2R w K - 0 1").unwrap();
        assert_eq!(board.piece_at("e2"), Some((Color::White, san::Piece::Pawn)));
        assert_eq!(board.piece_at("h1"), Some((Color::White, san::Piece::Rook)));
        assert_eq!(board.side_to_move(), Color::White);
    }
}
