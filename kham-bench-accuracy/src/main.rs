//! Word-boundary precision / recall / F1 benchmark for the kham segmenter.
//!
//! Reads `input|tok1|tok2|…` files from `kham-core/testdata/` (same format as
//! the integration test suite), segments each input with `Tokenizer::new()`,
//! and compares the resulting character-span sets against the gold spans.
//!
//! ```text
//! cargo run -p kham-bench-accuracy
//! cargo run -p kham-bench-accuracy -- --threshold 0.95
//! cargo run -p kham-bench-accuracy -- --verbose
//! ```

use std::collections::BTreeSet;
use std::path::PathBuf;

use clap::Parser;
use kham_core::token::TokenKind;
use kham_core::Tokenizer;

#[derive(Parser)]
#[command(
    name = "kham-bench-accuracy",
    about = "Word-boundary P/R/F1 benchmark for kham-core segmenter"
)]
struct Args {
    /// Directory containing *.txt testdata files (input|tok1|tok2|… format).
    #[arg(long, default_value = "kham-core/testdata")]
    testdata: PathBuf,

    /// Exit with code 1 if aggregate F1 falls below this threshold.
    #[arg(long)]
    threshold: Option<f64>,

    /// Print each case where system output differs from gold.
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Default)]
struct Metrics {
    cases: usize,
    tp: usize,
    fp: usize,
    fn_: usize,
}

impl Metrics {
    fn precision(&self) -> f64 {
        let d = self.tp + self.fp;
        if d == 0 { 1.0 } else { self.tp as f64 / d as f64 }
    }

    fn recall(&self) -> f64 {
        let d = self.tp + self.fn_;
        if d == 0 { 1.0 } else { self.tp as f64 / d as f64 }
    }

    fn f1(&self) -> f64 {
        let p = self.precision();
        let r = self.recall();
        if p + r == 0.0 { 0.0 } else { 2.0 * p * r / (p + r) }
    }

    fn merge(&mut self, other: &Metrics) {
        self.cases += other.cases;
        self.tp += other.tp;
        self.fp += other.fp;
        self.fn_ += other.fn_;
    }
}

struct Case<'a> {
    input: &'a str,
    gold: Vec<&'a str>,
}

fn parse_file(src: &str) -> Vec<Case<'_>> {
    src.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let mut parts = line.split('|');
            let input = parts.next()?;
            let gold: Vec<&str> = parts.collect();
            if gold.is_empty() { None } else { Some(Case { input, gold }) }
        })
        .collect()
}

/// Build char-span set for gold tokens, skipping whitespace gaps in `input`.
fn gold_spans(input: &str, gold: &[&str]) -> BTreeSet<(usize, usize)> {
    let chars: Vec<char> = input.chars().collect();
    let mut spans = BTreeSet::new();
    let mut pos = 0;
    for tok in gold {
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }
        let len = tok.chars().count();
        spans.insert((pos, pos + len));
        pos += len;
    }
    spans
}

fn evaluate(cases: &[Case<'_>], tokenizer: &Tokenizer, verbose: bool, label: &str) -> Metrics {
    let mut m = Metrics::default();
    for case in cases {
        let gold = gold_spans(case.input, &case.gold);

        let sys_tokens = tokenizer.segment(case.input);
        let sys: BTreeSet<(usize, usize)> = sys_tokens
            .iter()
            .filter(|t| !matches!(t.kind, TokenKind::Whitespace))
            .map(|t| (t.char_span.start, t.char_span.end))
            .collect();

        let tp = gold.intersection(&sys).count();
        let fp = sys.difference(&gold).count();
        let fn_ = gold.difference(&sys).count();

        if verbose && (fp > 0 || fn_ > 0) {
            let gold_fmt: Vec<String> = case.gold.iter().map(|s| format!("[{}]", s)).collect();
            let sys_fmt: Vec<String> = sys_tokens
                .iter()
                .filter(|t| !matches!(t.kind, TokenKind::Whitespace))
                .map(|t| format!("[{}]", t.text))
                .collect();
            eprintln!("  FAIL {}  {:?}", label, case.input);
            eprintln!("    gold:   {}", gold_fmt.join(" "));
            eprintln!("    system: {}", sys_fmt.join(" "));
        }

        m.cases += 1;
        m.tp += tp;
        m.fp += fp;
        m.fn_ += fn_;
    }
    m
}

fn main() {
    let args = Args::parse();
    let tokenizer = Tokenizer::new();

    let mut files: Vec<PathBuf> = std::fs::read_dir(&args.testdata)
        .unwrap_or_else(|e| panic!("cannot read testdata dir {:?}: {}", args.testdata, e))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .collect();
    files.sort();

    if files.is_empty() {
        eprintln!("No *.txt files found in {:?}", args.testdata);
        std::process::exit(2);
    }

    println!("kham-bench-accuracy — Thai word segmentation accuracy");
    println!();
    println!("Testdata: {} ({} files)", args.testdata.display(), files.len());
    println!();

    const W: usize = 24;
    let sep = "─".repeat(W + 49);
    println!("{:<W$} {:>6} {:>6} {:>5} {:>5}  {:>6} {:>6} {:>6}", "File", "Cases", "TP", "FP", "FN", "P", "R", "F1");
    println!("{sep}");

    let mut aggregate = Metrics::default();

    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy();
        let src = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read {:?}: {}", path, e));
        let cases = parse_file(&src);
        let m = evaluate(&cases, &tokenizer, args.verbose, &name);
        println!(
            "{:<W$} {:>6} {:>6} {:>5} {:>5}  {:>6.3} {:>6.3} {:>6.3}",
            name, m.cases, m.tp, m.fp, m.fn_, m.precision(), m.recall(), m.f1()
        );
        aggregate.merge(&m);
    }

    println!("{sep}");
    println!(
        "{:<W$} {:>6} {:>6} {:>5} {:>5}  {:>6.3} {:>6.3} {:>6.3}",
        "AGGREGATE",
        aggregate.cases, aggregate.tp, aggregate.fp, aggregate.fn_,
        aggregate.precision(), aggregate.recall(), aggregate.f1()
    );
    println!();

    if let Some(threshold) = args.threshold {
        let f1 = aggregate.f1();
        if f1 < threshold {
            eprintln!("FAILED — aggregate F1 {:.3} < threshold {:.3}", f1, threshold);
            std::process::exit(1);
        }
        println!("Passed — F1 {:.3} >= threshold {:.3}", f1, threshold);
    }
}
