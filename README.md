# sanlint

A command-line tool that checks whether chess moves are written in valid
SAN (Standard Algebraic Notation). It answers one question: is this move
text well-formed?

Every PGN viewer and every hand-written game log tends to accumulate small
notation mistakes: a missing capture file (`xd5` instead of `exd5`), a pawn
push to the last rank with no promotion piece, castling written with a
zero instead of a capital O, two check symbols stuck on the end of a move.
Most parsers either silently accept this or crash on it. `sanlint` is
narrower than a full PGN parser or a chess engine: it does not know about
board positions, so it cannot tell you whether a move is legal in context.
What it does check is everything the SAN grammar fixes on its own.

## Usage

```
sanlint [--lenient] [FILE]
```

Reads PGN movetext from FILE, or from stdin if no file is given. Headers,
comments, and variations are skipped; only move tokens are checked.

```
$ sanlint game.pgn
line 4: 'Nxe5+#' - more than one check/mate symbol
line 6: 'xd5' - pawn capture 'xd5' is missing its origin file
line 9: 'e8' - pawn move 'e8' reaches the last rank but has no promotion
checked 41 moves, 3 errors
```

Exit status is 0 if every move is valid, 1 otherwise.

You can also pipe in a bare move list:

```
$ echo "1. e4 e5 2. Nf3 Nc6 3. Bb5" | sanlint
checked 5 moves, 0 errors
```

### Strict by default, `--lenient` as an escape hatch

By default `sanlint` enforces standard SAN exactly:

- piece letters `K Q R B N` must be uppercase
- castling must be written `O-O` / `O-O-O` with a capital letter O
- a pawn capture must include its origin file (`exd5`, not `xd5`)
- promotions must use uppercase `Q R B N` after `=`
- a pawn move landing on rank 1 or 8 must include a promotion
- at most one check (`+`) or mate (`#`) symbol, and it must be last

Real-world PGN, especially anything typed by hand or exported from older
software, sometimes bends these rules in a few specific, well-known ways.
Pass `--lenient` to accept those too:

- `0-0` / `0-0-0` (digit zero) for castling
- a lowercase piece letter, e.g. `nf3` for `Nf3`
- a lowercase promotion letter, e.g. `e8=q`
- `:` in place of `x` for a capture, e.g. `Nxe5` written as `N:e5`
- a trailing `e.p.` or `ep` marker on an en passant capture

`--lenient` only widens what counts as valid syntax. It never suppresses
an error that would be reported anyway (a missing promotion is still a
missing promotion), and it never checks board legality, since that is out
of scope for this tool either way.

## Building

Standard library only, no dependencies:

```
cargo build --release
```

## What this does not do (yet)

`sanlint` has no board model, so it cannot check whether a move is legal
in the position it's played in, whether a disambiguator is actually
necessary, or whether a `+`/`#` matches the real result of the move. It
checks notation shape, not chess.
