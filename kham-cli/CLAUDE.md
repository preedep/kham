# kham-cli

CLI binary using `clap`. Exposes user-facing runtime flags only — not internal implementation details.

## Flags

| Flag | Purpose |
|------|---------|
| `--dict <FILE>` | Custom word list (plain text, one word per line) |
| `--sep <STR>` | Token separator (default: `\|`) |
| `--whitespace` | Include whitespace tokens in output |
| `--normalize` | Normalize text before segmenting |
| `--kind` | Append token kind: `กิน:Thai` |
| `--spans` | Append Unicode char span: `กิน:0-3` |
| `--fts` | Switch to `FtsTokenizer`; print one token per line with tab-separated fields: `text kind=KIND pos=POS ne=NE stop=BOOL syn=SYNONYMS` |
| `--soundex <ALGO>` | Phonetic encoding for `--fts` mode; valid: `lk82`, `udom83`, `metasound`. Emits soundex code into `syn=` field for Thai and Named tokens. No effect without `--fts`. |
| `--confidence` | Append `conf=<val>` per token in text output mode (e.g. `กิน:Thai:conf=0.90`). In FTS text mode, appends a `conf=<val>` tab-separated field. No effect with `--format json` or `--format csv` (confidence is always included there). |
| `--min-confidence <MIN>` | Filter output to tokens with confidence ≥ MIN (0.0–1.0). Works in both basic and FTS mode. |
| `--format <FORMAT>` | Output format: `text` (default), `json`, `csv`. `text` = current behaviour. `json` = one JSON array per input line. `csv` = header row then data rows (comma-separated, fields quoted if needed). |
| `--romanize` | Segment and romanize Thai text to RTGS Latin. Non-Thai tokens pass through unchanged. Incompatible with --fts. |

Combined `--kind --spans` produces `กิน:Thai:0-3`.

## Rules

- `--fts` is incompatible with `--dict` — warn and ignore `--dict` when both are given.
- `--fts` is intended for testing and inspecting the FTS pipeline (POS, NE, stopword metadata), not production use.

## Do NOT add

- `--freq-file` or `--no-freq` — frequency data is an internal DP scorer tiebreaker, not a user input. Users cannot meaningfully author replacement frequency data (requires corpus counts). If domain-tuned frequencies are ever needed, add `freq_tsv(data: &str)` to `TokenizerBuilder` first, then reconsider the CLI.
- Any flag that exposes internal scoring parameters or pipeline knobs not useful to an end user.
