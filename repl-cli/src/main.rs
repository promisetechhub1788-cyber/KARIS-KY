//! KARIS-KY REPL command-line interface.
//!
//! This binary provides a minimal REPL entry point for the KARIS-KY
//! workspace. It reads a single command from the process arguments and
//! reports it back to the user.
//!
//! # Documentation policy
//!
//! The workspace documentation build is run with
//! `RUSTDOCFLAGS='-D rustdoc::broken_intra_doc_links'` so that any stale or
//! broken intra-doc links fail CI. When adding doc comments, only reference
//! items that actually exist (for example [`parse_input`]) and verify with
//! `cargo doc --workspace --no-deps`.

/// Validates and normalizes a raw command-line input string.
///
/// The input is trimmed of surrounding whitespace. Empty (or
/// whitespace-only) input is rejected.
///
/// # Errors
///
/// Returns an error message when the trimmed input is empty.
fn parse_input(input: &str) -> Result<&str, &'static str> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err("command cannot be empty");
    }

    Ok(trimmed)
}

/// Entry point for the REPL CLI.
///
/// Reads the first positional argument as the command (defaulting to
/// `"status"`), validates it via [`parse_input`], and prints the result.
/// Exits with a non-zero status code when the input is invalid.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let command = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "status".to_string());

    match parse_input(&command) {
        Ok(command) => {
            println!("KARIS-KY REPL CLI");
            if command == "fund-batch" {
                let csv_path = match args.get(2) {
                    Some(path) => path,
                    None => {
                        eprintln!("usage: fund-batch <csv-path>");
                        std::process::exit(1);
                    }
                };
                if let Err(message) = run_fund_batch(csv_path) {
                    eprintln!("{message}");
                    std::process::exit(1);
                }
            } else {
                println!("Command received: {command}");
            }
        }
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fund_batch, parse_fund_batch_csv, parse_input, Address};

    #[test]
    fn parse_input_accepts_non_empty_commands() {
        assert_eq!(parse_input("  status  ").unwrap(), "status");
    }

    #[test]
    fn parse_input_rejects_empty_input() {
        assert!(parse_input("   ").is_err());
    }
}