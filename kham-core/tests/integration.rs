//! Integration tests for kham-core.
//!
//! These tests exercise the full pipeline end-to-end:
//!   normalize() → segment() → Vec<Token>
//!
//! Test data files live in `testdata/` next to this directory:
//!   basic.txt          — pure Thai, all words in built-in dict
//!   mixed_script.txt   — Thai + Latin + Number combinations
//!   normalization.txt  — idempotency of normalize() + segment()
//!
//! File format (one test case per non-comment line):
//!   input|tok1|tok2|…
//! Whitespace tokens are excluded (keep_whitespace=false default).

use kham_core::{TokenKind, Tokenizer};

// ---------------------------------------------------------------------------
// Test data helpers
// ---------------------------------------------------------------------------

/// Parse a testdata file into `(input, expected_token_texts)` pairs.
///
/// Lines starting with `#` and blank lines are skipped.
fn load_cases(path: &str) -> Vec<(String, Vec<String>)> {
    let content =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));

    content
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|line| {
            let mut parts = line.splitn(2, '|');
            let input = parts.next().unwrap_or("").to_string();
            let rest = parts.next().unwrap_or("");
            let expected: Vec<String> = rest.split('|').map(|s| s.to_string()).collect();
            (input, expected)
        })
        .collect()
}

/// Segment `input` and return just the token texts (no whitespace tokens).
fn segment_texts(tok: &Tokenizer, input: &str) -> Vec<String> {
    tok.segment(input)
        .into_iter()
        .filter(|t| t.kind != TokenKind::Whitespace)
        .map(|t| t.text.to_string())
        .collect()
}

/// Path to a testdata file relative to the workspace root.
fn testdata(name: &str) -> String {
    // CARGO_MANIFEST_DIR points to kham-core/ during tests.
    let dir = env!("CARGO_MANIFEST_DIR");
    format!("{dir}/testdata/{name}")
}

// ---------------------------------------------------------------------------
// Basic Thai segmentation (all words in built-in dict)
// ---------------------------------------------------------------------------

#[test]
fn basic_sentences_split_correctly() {
    let tok = Tokenizer::new();
    for (input, expected) in load_cases(&testdata("basic.txt")) {
        let got = segment_texts(&tok, &input);
        assert_eq!(
            got, expected,
            "basic.txt: wrong split for {input:?}\n  got:      {got:?}\n  expected: {expected:?}"
        );
    }
}

#[test]
fn basic_sentences_reconstruct_input() {
    let tok = Tokenizer::new();
    for (input, _) in load_cases(&testdata("basic.txt")) {
        let rebuilt: String = tok.segment(&input).iter().map(|t| t.text).collect();
        // With no whitespace in input, reconstruction must be exact.
        assert_eq!(
            rebuilt, input,
            "basic.txt: reconstruction failed for {input:?}"
        );
    }
}

#[test]
fn basic_sentences_all_tokens_are_thai_kind() {
    let tok = Tokenizer::new();
    for (input, _) in load_cases(&testdata("basic.txt")) {
        for token in tok.segment(&input) {
            assert_eq!(
                token.kind,
                TokenKind::Thai,
                "basic.txt: expected Thai kind for {token:?} in {input:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Mixed script
// ---------------------------------------------------------------------------

#[test]
fn mixed_script_split_correctly() {
    let tok = Tokenizer::new();
    for (input, expected) in load_cases(&testdata("mixed_script.txt")) {
        let got = segment_texts(&tok, &input);
        assert_eq!(
            got, expected,
            "mixed_script.txt: wrong split for {input:?}\n  got:      {got:?}\n  expected: {expected:?}"
        );
    }
}

#[test]
fn mixed_script_reconstructs_without_whitespace() {
    let tok = Tokenizer::new();
    for (input, _) in load_cases(&testdata("mixed_script.txt")) {
        // Reconstruction should equal input minus any spaces (default drops whitespace).
        let expected_rebuilt: String = input.chars().filter(|c| !c.is_whitespace()).collect();
        let rebuilt: String = tok.segment(&input).iter().map(|t| t.text).collect();
        assert_eq!(
            rebuilt, expected_rebuilt,
            "mixed_script.txt: reconstruction failed for {input:?}"
        );
    }
}

#[test]
fn mixed_script_number_spans_are_number_kind() {
    let tok = Tokenizer::new();
    // ธนาคาร100แห่ง — the "100" span must be Number kind.
    let tokens = tok.segment("ธนาคาร100แห่ง");
    let num = tokens.iter().find(|t| t.text == "100");
    assert!(num.is_some(), "no Number token found");
    assert_eq!(num.unwrap().kind, TokenKind::Number);
}

#[test]
fn mixed_script_latin_spans_are_latin_kind() {
    let tok = Tokenizer::new();
    let tokens = tok.segment("สวัสดีhello");
    let lat = tokens.iter().find(|t| t.text == "hello");
    assert!(lat.is_some(), "no Latin token found");
    assert_eq!(lat.unwrap().kind, TokenKind::Latin);
}

// ---------------------------------------------------------------------------
// Normalization — idempotency on canonical input
// ---------------------------------------------------------------------------

#[test]
fn normalization_idempotent_on_canonical_input() {
    let tok = Tokenizer::new();
    for (input, expected) in load_cases(&testdata("normalization.txt")) {
        // normalize() on already-canonical text must not change it.
        let normalized = tok.normalize(&input);
        assert_eq!(
            normalized, input,
            "normalization.txt: normalize() changed canonical input {input:?}"
        );
        // Then segment must still produce the right splits.
        let got = segment_texts(&tok, &normalized);
        assert_eq!(
            got, expected,
            "normalization.txt: wrong split after normalize() for {input:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Normalization — malformed input (code-level, not file-based)
// ---------------------------------------------------------------------------

#[test]
fn normalize_then_segment_deduplicates_tone_marks() {
    let tok = Tokenizer::new();
    // กิน with doubled tone on a consonant — normalize removes the duplicate.
    // Insert ไป (U+0E44 U+0E1B) with doubled tone: ไ+ป+อ่+อ่
    // ไป is in the dict, so after dedup it should segment as one Thai token.
    let doubled_tone = "\u{0E44}\u{0E1B}\u{0E48}\u{0E48}"; // ไป + อ่ + อ่
    let normalized = tok.normalize(doubled_tone);
    // Tone dedup keeps only the last อ่
    assert_eq!(normalized, "\u{0E44}\u{0E1B}\u{0E48}"); // ไป่
    let tokens = tok.segment(&normalized);
    let rebuilt: String = tokens.iter().map(|t| t.text).collect();
    assert_eq!(rebuilt, normalized);
}

#[test]
fn normalize_then_segment_composes_sara_am() {
    let tok = Tokenizer::new();
    // น้ำ decomposed: น (U+0E19) + ้ (U+0E49) + อํ (U+0E4D) + อา (U+0E32)
    // normalize() composes อํ+อา → อำ, giving น้ำ (U+0E19 U+0E49 U+0E33)
    let decomposed = "\u{0E19}\u{0E49}\u{0E4D}\u{0E32}"; // น + ้ + อํ + อา
    let normalized = tok.normalize(decomposed);
    assert_eq!(normalized, "\u{0E19}\u{0E49}\u{0E33}"); // น้ำ
                                                        // น้ำ is in the built-in dict — should be one Thai token.
    let tokens = tok.segment(&normalized);
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].text, "\u{0E19}\u{0E49}\u{0E33}");
    assert_eq!(tokens[0].kind, TokenKind::Thai);
}

// ---------------------------------------------------------------------------
// Edge cases (CLAUDE.md required list)
// ---------------------------------------------------------------------------

#[test]
fn edge_empty_string() {
    let tok = Tokenizer::new();
    assert!(tok.segment("").is_empty());
    assert_eq!(tok.normalize(""), "");
}

#[test]
fn edge_single_thai_char() {
    let tok = Tokenizer::new();
    let tokens = tok.segment("ก");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].text, "ก");
}

#[test]
fn edge_single_latin_char() {
    let tok = Tokenizer::new();
    let tokens = tok.segment("A");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::Latin);
}

#[test]
fn edge_zero_width_chars_do_not_panic() {
    let tok = Tokenizer::new();
    // Zero-width joiner (U+200D) and zero-width non-joiner (U+200C)
    let input = "กิน\u{200D}ข้าว";
    let tokens = tok.segment(input);
    let rebuilt: String = tokens.iter().map(|t| t.text).collect();
    assert_eq!(rebuilt, input);
}

#[test]
fn edge_sara_waw_floating_vowel() {
    // สระลอย edge case: ensure a lead vowel not followed by a consonant
    // (lone leading vowel) does not panic or produce an empty token.
    let tok = Tokenizer::new();
    let input = "\u{0E40}"; // lone เ
    let tokens = tok.segment(input);
    assert!(!tokens.is_empty());
    let rebuilt: String = tokens.iter().map(|t| t.text).collect();
    assert_eq!(rebuilt, input);
}

#[test]
fn edge_repeated_tone_marks_only() {
    // A string of only tone marks should not panic.
    let tok = Tokenizer::new();
    let input = "\u{0E48}\u{0E49}\u{0E4A}"; // อ่ อ้ อ๊
    let tokens = tok.segment(input);
    let rebuilt: String = tokens.iter().map(|t| t.text).collect();
    assert_eq!(rebuilt, input);
}

#[test]
fn edge_all_whitespace() {
    let tok = Tokenizer::new();
    // Default: whitespace dropped
    assert!(tok.segment("   \t\n").is_empty());
    // keep_whitespace: whitespace preserved
    let tokens = Tokenizer::builder()
        .keep_whitespace(true)
        .build()
        .segment("   ");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::Whitespace);
}

#[test]
fn edge_mixed_thai_number_thai_no_spaces() {
    // Classic CLAUDE.md example — must not panic and must reconstruct.
    let tok = Tokenizer::new();
    let input = "ธนาคาร100แห่ง";
    let rebuilt: String = tok.segment(input).iter().map(|t| t.text).collect();
    assert_eq!(rebuilt, input);
}

// ---------------------------------------------------------------------------
// Span / byte-offset invariants (full pipeline)
// ---------------------------------------------------------------------------

#[test]
fn all_spans_valid_utf8_boundaries() {
    let tok = Tokenizer::new();
    let cases = ["กินข้าวกับปลา", "ธนาคาร100แห่ง", "สวัสดีhello123", "คนที่นี่ไปมา"];
    for input in cases {
        for token in tok.segment(input) {
            assert!(
                input.is_char_boundary(token.span.start),
                "span.start not a char boundary in {input:?}: {token:?}"
            );
            assert!(
                input.is_char_boundary(token.span.end),
                "span.end not a char boundary in {input:?}: {token:?}"
            );
            assert_eq!(
                &input[token.span.clone()],
                token.text,
                "span/text mismatch in {input:?}: {token:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Generic testdata file-discovery runner
// ---------------------------------------------------------------------------

/// Discover every `*.txt` file in `testdata/` and assert segment() output
/// against every case it contains.
///
/// This test runs automatically when new `.txt` files are added to
/// `testdata/` — no changes to this file are needed.
#[test]
fn all_testdata_files() {
    let dir = format!("{}/testdata", env!("CARGO_MANIFEST_DIR"));
    let tok = Tokenizer::new();

    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read testdata dir {dir}: {e}"))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |x| x == "txt"))
        .collect();

    // Sort for deterministic test order.
    entries.sort_by_key(|e| e.path());

    assert!(
        !entries.is_empty(),
        "testdata/ contains no .txt files — expected at least basic.txt"
    );

    let mut total_cases = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for entry in &entries {
        let path = entry.path();
        let file_name = path.file_name().unwrap().to_string_lossy().into_owned();

        for (input, expected) in load_cases(&path.to_string_lossy()) {
            total_cases += 1;
            let got = segment_texts(&tok, &input);
            if got != expected {
                failures.push(format!(
                    "{file_name}: {input:?}\n    got:      {got:?}\n    expected: {expected:?}"
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} / {} case(s) failed:\n{}",
        failures.len(),
        total_cases,
        failures.join("\n")
    );
}

#[test]
fn keep_whitespace_spans_are_contiguous() {
    let tok = Tokenizer::builder().keep_whitespace(true).build();
    let input = "กิน ข้าว 100 hello";
    let tokens = tok.segment(input);
    for w in tokens.windows(2) {
        assert_eq!(
            w[0].span.end, w[1].span.start,
            "gap between tokens in {input:?}: {:?} and {:?}",
            w[0], w[1]
        );
    }
}
