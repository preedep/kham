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
use std::time::Instant;

use clap::Parser;
use colored::Colorize;
use kham_core::Tokenizer;
use log::{debug, info, warn};

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
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format(|buf, record| {
            use std::io::Write;

            let level = match record.level() {
                log::Level::Error => format!("{:<5}", record.level()).red().bold(),
                log::Level::Warn => format!("{:<5}", record.level()).yellow().bold(),
                log::Level::Info => format!("{:<5}", record.level()).green().bold(),
                log::Level::Debug => format!("{:<5}", record.level()).cyan(),
                log::Level::Trace => format!("{:<5}", record.level()).magenta(),
            };

            let ts = buf.timestamp_micros().to_string().dimmed();
            let target = format!("[{}]", record.target()).dimmed();
            let message = record.args().to_string();

            // Colour the message body for WARN/ERROR to make it stand out.
            let message = match record.level() {
                log::Level::Error => message.red().to_string(),
                log::Level::Warn => message.yellow().to_string(),
                _ => message,
            };

            writeln!(buf, "{ts} {level} {target} {message}")
        })
        .init();

    let cli = Cli::parse();

    debug!("CLI args: {:?}", cli);

    // Build the tokenizer.
    debug!("Building tokenizer (keep_whitespace={})", cli.whitespace);
    let t0 = Instant::now();

    let mut builder = Tokenizer::builder().keep_whitespace(cli.whitespace);
    if let Some(dict_path) = &cli.dict {
        info!("Loading custom dictionary: {}", dict_path);
        builder = match builder.dict_file(dict_path) {
            Ok(b) => {
                debug!("Custom dictionary loaded successfully");
                b
            }
            Err(e) => {
                eprintln!("kham: error: {e}");
                std::process::exit(1);
            }
        };
    }
    let tokenizer = builder.build();
    debug!(
        "Tokenizer ready ({:.3}ms)",
        t0.elapsed().as_secs_f64() * 1000.0
    );

    match cli.text {
        // Text supplied as a positional argument.
        Some(ref text) => {
            debug!("Mode: positional argument ({} bytes)", text.len());
            process_line(&tokenizer, text, &cli);
        }
        // No argument — read stdin line-by-line (pipeline / interactive mode).
        None => {
            debug!("Mode: stdin");
            let stdin = io::stdin();
            let mut line_count = 0usize;
            for line in stdin.lock().lines() {
                match line {
                    Ok(text) => {
                        line_count += 1;
                        debug!("stdin line {}: {} bytes", line_count, text.len());
                        process_line(&tokenizer, &text, &cli);
                    }
                    Err(e) => {
                        eprintln!("kham: read error: {e}");
                        std::process::exit(1);
                    }
                }
            }
            debug!("stdin: processed {} line(s)", line_count);
        }
    }
}

/// Normalize (optionally), segment, and print one line of output.
fn process_line(tokenizer: &Tokenizer, raw: &str, cli: &Cli) {
    debug!("process_line: input={:?} ({} bytes)", raw, raw.len());

    // Normalize into an owned String when requested; otherwise borrow raw.
    let normalized;
    let text: &str = if cli.normalize {
        let t0 = Instant::now();
        normalized = tokenizer.normalize(raw);
        debug!(
            "normalize: {:?} → {:?} ({:.3}ms)",
            raw,
            normalized,
            t0.elapsed().as_secs_f64() * 1000.0
        );
        if normalized != raw {
            info!("normalize: text changed after normalization");
        }
        &normalized
    } else {
        raw
    };

    let t0 = Instant::now();
    let tokens = tokenizer.segment(text);
    let elapsed_us = t0.elapsed().as_secs_f64() * 1_000_000.0;

    debug!(
        "segment: {} token(s) in {:.2}µs  [{:.1} MiB/s]",
        tokens.len(),
        elapsed_us,
        if elapsed_us > 0.0 {
            text.len() as f64 / elapsed_us
        } else {
            0.0
        }
    );

    if log::log_enabled!(log::Level::Debug) {
        for (i, t) in tokens.iter().enumerate() {
            debug!(
                "  token[{:02}] {:?}  span={}..{}  text={:?}",
                i, t.kind, t.span.start, t.span.end, t.text,
            );
        }
    }

    let unknown_count = tokens
        .iter()
        .filter(|t| t.kind == kham_core::TokenKind::Unknown)
        .count();
    if unknown_count > 0 {
        warn!("segment: {} unknown token(s) in {:?}", unknown_count, text);
    }

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
