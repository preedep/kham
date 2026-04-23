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
| `--fts` | Switch to `FtsTokenizer`; print one token per line with tab-separated fields: `text kind=KIND pos=POS ne=NE stop=BOOL` |

Combined `--kind --spans` produces `กิน:Thai:0-3`.

## Rules

- `--fts` is incompatible with `--dict` — warn and ignore `--dict` when both are given.
- `--fts` is intended for testing and inspecting the FTS pipeline (POS, NE, stopword metadata), not production use.

## Do NOT add

- `--freq-file` or `--no-freq` — frequency data is an internal DP scorer tiebreaker, not a user input. Users cannot meaningfully author replacement frequency data (requires corpus counts). If domain-tuned frequencies are ever needed, add `freq_tsv(data: &str)` to `TokenizerBuilder` first, then reconsider the CLI.
- Any flag that exposes internal scoring parameters or pipeline knobs not useful to an end user.
