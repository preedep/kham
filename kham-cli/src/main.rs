//! `kham` — Thai word segmentation CLI.
//!
//! ```text
//! USAGE:
//!     kham [OPTIONS] [TEXT]
//!
//! ARGS:
//!     [TEXT]    Thai text to segment. Reads from stdin if omitted.
//!
//! OPTIONS:
//!     -d, --dict <FILE>    Path to a custom word-list file (newline-separated)
//!     -s, --sep <SEP>      Output separator between tokens [default: "|"]
//!     -w, --whitespace     Include whitespace tokens in output
//!     -n, --normalize      Run normalize() before segmenting
//!     -k, --kind           Append token kind after each token (e.g. กิน:Thai)
//!     -h, --help           Print help information
//!     -V, --version        Print version information
//! ```

use std::io::{self, BufRead};

use clap::Parser;
use kham_core::Tokenizer;

#[derive(Parser, Debug)]
#[command(
    name = "kham",
    version,
    about = "Thai word segmentation engine",
    long_about = None,
)]
struct Cli {
    /// Thai text to segment. Reads from stdin line-by-line if omitted.
    text: Option<String>,

    /// Path to a custom word-list file (newline-separated words).
    #[arg(short, long, value_name = "FILE")]
    dict: Option<String>,

    /// Separator printed between tokens.
    #[arg(short, long, default_value = "|")]
    sep: String,

    /// Include whitespace tokens in output.
    #[arg(short, long)]
    whitespace: bool,

    /// Normalize text before segmenting (tone dedup + Sara Am composition).
    #[arg(short, long)]
    normalize: bool,

    /// Append the token kind after each token text (e.g. กิน:Thai).
    #[arg(short, long)]
    kind: bool,
}

fn main() {
    let cli = Cli::parse();

    // Build the tokenizer.
    let mut builder = Tokenizer::builder().keep_whitespace(cli.whitespace);
    if let Some(dict_path) = &cli.dict {
        builder = match builder.dict_file(dict_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("kham: error: {e}");
                std::process::exit(1);
            }
        };
    }
    let tokenizer = builder.build();

    match cli.text {
        // Text supplied as a positional argument.
        Some(ref text) => {
            process_line(&tokenizer, text, &cli);
        }
        // No argument — read stdin line-by-line (pipeline / interactive mode).
        None => {
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                match line {
                    Ok(text) => process_line(&tokenizer, &text, &cli),
                    Err(e) => {
                        eprintln!("kham: read error: {e}");
                        std::process::exit(1);
                    }
                }
            }
        }
    }
}

/// Normalize (optionally), segment, and print one line of output.
fn process_line(tokenizer: &Tokenizer, raw: &str, cli: &Cli) {
    // Normalize into an owned String when requested; otherwise borrow raw.
    let normalized;
    let text: &str = if cli.normalize {
        normalized = tokenizer.normalize(raw);
        &normalized
    } else {
        raw
    };

    let tokens = tokenizer.segment(text);

    let parts: Vec<String> = tokens
        .iter()
        .map(|t| {
            if cli.kind {
                format!("{}:{:?}", t.text, t.kind)
            } else {
                t.text.to_string()
            }
        })
        .collect();

    println!("{}", parts.join(&cli.sep));
}
