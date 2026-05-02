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
//!     -d, --dict <FILE>           Path to a custom word-list file (newline-separated)
//!     -s, --sep <SEP>             Output separator between tokens [default: "|"]
//!     -w, --whitespace            Include whitespace tokens in output
//!     -n, --normalize             Run normalize() before segmenting
//!     -k, --kind                  Append token kind after each token (e.g. กิน:Thai)
//!         --spans                 Append char span after each token (e.g. กิน:0-3)
//!         --fts                   Run the FTS pipeline; print one token per line with
//!                                 kind, POS, NE, and stopword metadata
//!         --confidence            Show confidence score alongside each token in text
//!                                 output mode (e.g. กิน:Thai:conf=0.90)
//!         --min-confidence <MIN>  Only output tokens with confidence ≥ MIN (0.0–1.0)
//!         --format <FORMAT>       Output format: text (default), json, csv
//!     -h, --help                  Print help information
//!     -V, --version               Print version information
//! ```

use std::io::{self, BufRead};
use std::time::Instant;

use clap::Parser;
use colored::Colorize;
use kham_core::fts::FtsTokenizer;
use kham_core::soundex::SoundexAlgorithm;
use kham_core::{TokenKind, TokenStream, Tokenizer};
use log::{debug, info, warn};

// ---------------------------------------------------------------------------
// Token kind helper (shared by basic and FTS output paths)
// ---------------------------------------------------------------------------

fn kind_str(kind: TokenKind) -> &'static str {
    match kind {
        TokenKind::Thai => "Thai",
        TokenKind::Latin => "Latin",
        TokenKind::Number => "Number",
        TokenKind::Punctuation => "Punctuation",
        TokenKind::Emoji => "Emoji",
        TokenKind::Whitespace => "Whitespace",
        TokenKind::Unknown => "Unknown",
        TokenKind::Named(ne) => ne.as_str(),
    }
}

// ---------------------------------------------------------------------------
// CSV quoting helper
// ---------------------------------------------------------------------------

fn csv_quote(s: &str) -> String {
    if s.contains(',') || s.contains('"') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

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
    /// Not supported with --fts (custom dict is ignored in FTS mode).
    #[arg(short, long, value_name = "FILE")]
    dict: Option<String>,

    /// Separator printed between tokens (basic mode only).
    #[arg(short, long, default_value = "|")]
    sep: String,

    /// Include whitespace tokens in output (basic mode only).
    #[arg(short, long)]
    whitespace: bool,

    /// Normalize text before segmenting (basic mode only; FTS always normalizes).
    #[arg(short, long)]
    normalize: bool,

    /// Append the token kind after each token text (basic mode only, e.g. กิน:Thai).
    #[arg(short, long)]
    kind: bool,

    /// Append the Unicode char span after each token text (basic mode only, e.g. กิน:0-3).
    #[arg(long)]
    spans: bool,

    /// Run the full FTS pipeline (FtsTokenizer).
    ///
    /// Prints one token per line with tab-separated fields:
    ///   text  kind=KIND  pos=POS  ne=NE  stop=BOOL  syn=SYNONYMS
    ///
    /// KIND is the token script category (Thai, Latin, Number, Person, Place, Org, …).
    /// POS is the part-of-speech tag (Verb, Noun, Adj, …) or "-" for OOV/non-Thai.
    /// NE  is the named entity category (Person, Place, Org) or "-" if not an NE.
    /// BOOL is true if the token matched the built-in stopword list.
    /// SYNONYMS is a comma-separated list of synonym strings, or "-" if none.
    ///
    /// Pipe through `column -t` for aligned columns:
    ///   kham --fts "ทักษิณเดินทางไปไทย" | column -t
    #[arg(long)]
    fts: bool,

    /// Phonetic encoding algorithm for FTS mode (lk82, udom83, metasound).
    ///
    /// Emits the soundex code into the `syn=` field for Thai and Named tokens.
    /// Has no effect without --fts.
    ///
    /// Example: kham --fts --soundex lk82 "กินข้าวกับปลา"
    #[arg(long, value_name = "ALGO")]
    soundex: Option<String>,

    /// Show confidence score alongside each token in text output mode.
    /// Has no effect when --format json or --format csv (confidence is always included there).
    #[arg(long)]
    confidence: bool,

    /// Only output tokens with confidence ≥ MIN (0.0–1.0).
    #[arg(long, value_name = "MIN")]
    min_confidence: Option<f32>,

    /// Output format: text (default), json, csv.
    #[arg(long, default_value = "text", value_name = "FORMAT")]
    format: String,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

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

    // Validate --format
    match cli.format.as_str() {
        "text" | "json" | "csv" => {}
        other => {
            eprintln!("kham: unknown format {other:?}; valid: text, json, csv");
            std::process::exit(1);
        }
    }

    if cli.soundex.is_some() && !cli.fts {
        warn!("--soundex has no effect without --fts");
    }

    if cli.confidence && (cli.format == "json" || cli.format == "csv") {
        warn!(
            "--confidence has no effect with --format {}; confidence is always included",
            cli.format
        );
    }

    if cli.fts {
        run_fts_mode(&cli);
    } else {
        run_basic_mode(&cli);
    }
}

// ---------------------------------------------------------------------------
// Basic segmentation mode
// ---------------------------------------------------------------------------

fn run_basic_mode(cli: &Cli) {
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

    // Print CSV header once before any lines
    if cli.format == "csv" {
        println!("text,kind,char_start,char_end,byte_start,byte_end,confidence");
    }

    match cli.text {
        Some(ref text) => {
            debug!("Mode: positional argument ({} bytes)", text.len());
            process_line(&tokenizer, text, cli);
        }
        None => {
            debug!("Mode: stdin");
            let stdin = io::stdin();
            let mut line_count = 0usize;
            for line in stdin.lock().lines() {
                match line {
                    Ok(text) => {
                        line_count += 1;
                        debug!("stdin line {}: {} bytes", line_count, text.len());
                        process_line(&tokenizer, &text, cli);
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

    // Collect tokens, filtering by min_confidence if requested
    let tokens: Vec<_> = if let Some(min) = cli.min_confidence {
        let mut stream: TokenStream<'_> = tokenizer.segment_stream(text);
        let mut collected = Vec::new();
        while let Some(tok) = stream.next_above_confidence(min) {
            collected.push(tok);
        }
        collected
    } else {
        tokenizer.segment(text)
    };

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
        .filter(|t| t.kind == TokenKind::Unknown)
        .count();
    if unknown_count > 0 {
        warn!("segment: {} unknown token(s) in {:?}", unknown_count, text);
    }

    match cli.format.as_str() {
        "json" => {
            let arr: Vec<serde_json::Value> = tokens
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "text": t.text,
                        "kind": kind_str(t.kind),
                        "char_start": t.char_span.start,
                        "char_end": t.char_span.end,
                        "byte_start": t.span.start,
                        "byte_end": t.span.end,
                        "confidence": t.confidence,
                    })
                })
                .collect();
            println!("{}", serde_json::json!(arr));
        }
        "csv" => {
            for t in &tokens {
                println!(
                    "{},{},{},{},{},{},{}",
                    csv_quote(t.text),
                    csv_quote(kind_str(t.kind)),
                    t.char_span.start,
                    t.char_span.end,
                    t.span.start,
                    t.span.end,
                    t.confidence,
                );
            }
        }
        _ => {
            // text format (default)
            let parts: Vec<String> = tokens
                .iter()
                .map(|t| {
                    let mut s = match (cli.kind, cli.spans) {
                        (true, true) => format!(
                            "{}:{}:{}-{}",
                            t.text,
                            kind_str(t.kind),
                            t.char_span.start,
                            t.char_span.end
                        ),
                        (true, false) => format!("{}:{}", t.text, kind_str(t.kind)),
                        (false, true) => {
                            format!("{}:{}-{}", t.text, t.char_span.start, t.char_span.end)
                        }
                        (false, false) => t.text.to_string(),
                    };
                    if cli.confidence {
                        s.push_str(&format!(":conf={:.2}", t.confidence));
                    }
                    s
                })
                .collect();
            println!("{}", parts.join(&cli.sep));
        }
    }
}

// ---------------------------------------------------------------------------
// FTS pipeline mode
// ---------------------------------------------------------------------------

fn parse_soundex_algo(s: &str) -> SoundexAlgorithm {
    match s.to_lowercase().as_str() {
        "lk82" => SoundexAlgorithm::Lk82,
        "udom83" => SoundexAlgorithm::Udom83,
        "metasound" => SoundexAlgorithm::MetaSound,
        other => {
            eprintln!("kham: unknown soundex algorithm {other:?}; valid: lk82, udom83, metasound");
            std::process::exit(1);
        }
    }
}

fn run_fts_mode(cli: &Cli) {
    if cli.dict.is_some() {
        warn!("--dict is not supported with --fts and will be ignored");
    }

    debug!("Building FtsTokenizer");
    let t0 = Instant::now();
    let fts = if let Some(algo_str) = &cli.soundex {
        let algo = parse_soundex_algo(algo_str);
        debug!("FtsTokenizer: soundex={:?}", algo);
        FtsTokenizer::builder().soundex(algo).build()
    } else {
        FtsTokenizer::new()
    };
    debug!(
        "FtsTokenizer ready ({:.3}ms)",
        t0.elapsed().as_secs_f64() * 1000.0
    );

    // Print CSV header once before any lines
    if cli.format == "csv" {
        println!("text,position,kind,is_stop,pos,ne,synonyms,confidence");
    }

    match cli.text {
        Some(ref text) => {
            debug!("FTS mode: positional argument ({} bytes)", text.len());
            process_fts_line(&fts, text, cli);
        }
        None => {
            debug!("FTS mode: stdin");
            let stdin = io::stdin();
            let mut line_count = 0usize;
            for line in stdin.lock().lines() {
                match line {
                    Ok(text) => {
                        line_count += 1;
                        debug!("stdin line {}: {} bytes", line_count, text.len());
                        process_fts_line(&fts, &text, cli);
                    }
                    Err(e) => {
                        eprintln!("kham: read error: {e}");
                        std::process::exit(1);
                    }
                }
            }
            debug!("FTS mode: processed {} line(s)", line_count);
        }
    }
}

/// Run the FTS pipeline on one input line and print one token per line.
///
/// Output format (tab-separated):
/// ```text
/// TEXT    kind=KIND    pos=POS    ne=NE    stop=BOOL
/// ```
///
/// Pipe through `column -t` for aligned columns.
fn process_fts_line(fts: &FtsTokenizer, text: &str, cli: &Cli) {
    let mut tokens = fts.segment_for_fts(text);

    if let Some(min) = cli.min_confidence {
        tokens.retain(|t| t.confidence >= min);
    }

    match cli.format.as_str() {
        "json" => {
            let arr: Vec<serde_json::Value> = tokens
                .iter()
                .map(|t| {
                    let pos = t.pos.map(|p| p.as_str());
                    let ne = t.ne.map(|n| n.as_str());
                    serde_json::json!({
                        "text": t.text,
                        "position": t.position,
                        "kind": kind_str(t.kind),
                        "is_stop": t.is_stop,
                        "pos": pos,
                        "ne": ne,
                        "synonyms": t.synonyms,
                        "trigrams": t.trigrams,
                        "confidence": t.confidence,
                    })
                })
                .collect();
            println!("{}", serde_json::json!(arr));
        }
        "csv" => {
            for t in &tokens {
                let pos = t.pos.map(|p| p.as_str()).unwrap_or("-");
                let ne = t.ne.map(|n| n.as_str()).unwrap_or("-");
                let synonyms = t.synonyms.join(" ");
                println!(
                    "{},{},{},{},{},{},{},{}",
                    csv_quote(&t.text),
                    t.position,
                    csv_quote(kind_str(t.kind)),
                    t.is_stop,
                    csv_quote(pos),
                    csv_quote(ne),
                    csv_quote(&synonyms),
                    t.confidence,
                );
            }
        }
        _ => {
            // text format (default)
            for t in &tokens {
                let pos = t.pos.map(|p| p.as_str()).unwrap_or("-");
                let ne = t.ne.map(|n| n.as_str()).unwrap_or("-");
                let syn = if t.synonyms.is_empty() {
                    "-".to_string()
                } else {
                    t.synonyms.join(",")
                };
                if cli.confidence {
                    println!(
                        "{}\tkind={}\tpos={}\tne={}\tstop={}\tsyn={}\tconf={:.2}",
                        t.text,
                        kind_str(t.kind),
                        pos,
                        ne,
                        t.is_stop,
                        syn,
                        t.confidence,
                    );
                } else {
                    println!(
                        "{}\tkind={}\tpos={}\tne={}\tstop={}\tsyn={}",
                        t.text,
                        kind_str(t.kind),
                        pos,
                        ne,
                        t.is_stop,
                        syn,
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use kham_core::{token::NamedEntityKind, TokenKind};

    #[test]
    fn kind_str_all_variants() {
        assert_eq!(kind_str(TokenKind::Thai), "Thai");
        assert_eq!(kind_str(TokenKind::Latin), "Latin");
        assert_eq!(kind_str(TokenKind::Number), "Number");
        assert_eq!(kind_str(TokenKind::Punctuation), "Punctuation");
        assert_eq!(kind_str(TokenKind::Emoji), "Emoji");
        assert_eq!(kind_str(TokenKind::Whitespace), "Whitespace");
        assert_eq!(kind_str(TokenKind::Unknown), "Unknown");
        assert_eq!(
            kind_str(TokenKind::Named(NamedEntityKind::Person)),
            "Person"
        );
        assert_eq!(kind_str(TokenKind::Named(NamedEntityKind::Place)), "Place");
        assert_eq!(kind_str(TokenKind::Named(NamedEntityKind::Org)), "Org");
    }

    #[test]
    fn parse_soundex_algo_valid_inputs() {
        assert!(matches!(parse_soundex_algo("lk82"), SoundexAlgorithm::Lk82));
        assert!(matches!(parse_soundex_algo("LK82"), SoundexAlgorithm::Lk82));
        assert!(matches!(
            parse_soundex_algo("udom83"),
            SoundexAlgorithm::Udom83
        ));
        assert!(matches!(
            parse_soundex_algo("UDOM83"),
            SoundexAlgorithm::Udom83
        ));
        assert!(matches!(
            parse_soundex_algo("metasound"),
            SoundexAlgorithm::MetaSound
        ));
        assert!(matches!(
            parse_soundex_algo("METASOUND"),
            SoundexAlgorithm::MetaSound
        ));
    }

    #[test]
    fn csv_quote_no_special_chars() {
        assert_eq!(csv_quote("hello"), "hello");
        assert_eq!(csv_quote("Thai"), "Thai");
    }

    #[test]
    fn csv_quote_with_comma() {
        assert_eq!(csv_quote("a,b"), "\"a,b\"");
    }

    #[test]
    fn csv_quote_with_quote() {
        assert_eq!(csv_quote("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}
