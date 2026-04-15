//! `kham` — Thai word segmentation CLI.
//!
//! ```text
//! USAGE:
//!     kham [OPTIONS] <TEXT>
//!
//! ARGS:
//!     <TEXT>    Thai text to segment
//!
//! OPTIONS:
//!     -d, --dict <FILE>    Path to a custom word-list file (newline-separated)
//!     -s, --sep <SEP>      Output separator between tokens [default: "|"]
//!     -w, --whitespace     Include whitespace tokens in output
//!     -h, --help           Print help information
//!     -V, --version        Print version information
//! ```

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
    /// Thai text to segment.
    text: String,

    /// Path to a custom word-list file (newline-separated words).
    #[arg(short, long, value_name = "FILE")]
    dict: Option<String>,

    /// Separator printed between tokens.
    #[arg(short, long, default_value = "|")]
    sep: String,

    /// Include whitespace tokens in output.
    #[arg(short, long)]
    whitespace: bool,
}

fn main() {
    let cli = Cli::parse();

    let mut builder = Tokenizer::builder().keep_whitespace(cli.whitespace);

    if let Some(dict_path) = &cli.dict {
        builder = match builder.dict_file(dict_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        };
    }

    let tokenizer = builder.build();
    let tokens = tokenizer.segment(&cli.text);

    let parts: Vec<&str> = tokens.iter().map(|t| t.text).collect();
    println!("{}", parts.join(&cli.sep));
}
