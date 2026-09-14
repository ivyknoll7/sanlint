mod pgn;
mod san;

use std::env;
use std::fs;
use std::io::{self, Read};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut lenient = false;
    let mut path: Option<String> = None;

    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--lenient" => lenient = true,
            "-h" | "--help" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            other => {
                if path.is_some() {
                    eprintln!("sanlint: unexpected argument '{other}'");
                    print_usage();
                    return ExitCode::FAILURE;
                }
                path = Some(other.to_string());
            }
        }
    }

    let input = match &path {
        Some(p) => match fs::read_to_string(p) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("sanlint: cannot read '{p}': {e}");
                return ExitCode::FAILURE;
            }
        },
        None => {
            let mut buf = String::new();
            if let Err(e) = io::stdin().read_to_string(&mut buf) {
                eprintln!("sanlint: cannot read stdin: {e}");
                return ExitCode::FAILURE;
            }
            buf
        }
    };

    let tokens = pgn::tokenize(&input);
    let mut checked: u32 = 0;
    let mut errors: u32 = 0;

    for token in &tokens {
        let word = pgn::strip_move_number(&token.text);
        if word.is_empty() || pgn::is_result(word) || pgn::is_nag(word) {
            continue;
        }
        checked += 1;
        if let Err(e) = san::validate(word, lenient) {
            errors += 1;
            eprintln!("line {}: '{}' - {}", token.line, word, e);
        }
    }

    println!(
        "checked {checked} move{}, {errors} error{}",
        if checked == 1 { "" } else { "s" },
        if errors == 1 { "" } else { "s" },
    );

    if errors > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_usage() {
    eprintln!("usage: sanlint [--lenient] [FILE]");
    eprintln!();
    eprintln!("Validates SAN move notation in PGN movetext. Reads FILE, or stdin if");
    eprintln!("no FILE is given. Strict by default; --lenient relaxes a few common");
    eprintln!("non-standard forms (see README.md).");
}
