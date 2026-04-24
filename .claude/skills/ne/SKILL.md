# Named Entity Recognition — kham-core

Specialist for `kham-core/src/ne.rs` — gazetteer-based Person/Place/Org tagging of segmented Thai tokens.

## Approach: Gazetteer (Word-List Lookup)

No ML model. Tagging is a post-processing pass: after segmentation, each `TokenKind::Thai` token is checked against a `BTreeMap`; hits are relabeled to `TokenKind::Named(NamedEntityKind)`. Token boundaries are never changed.

## Entity Categories

| Tag | `NamedEntityKind` | Examples |
|-----|-------------------|---------|
| `PERSON` | `NamedEntityKind::Person` | ทักษิณ, สุเทพ, ประยุทธ์ |
| `PLACE` | `NamedEntityKind::Place` | ไทย, กรุงเทพ, เชียงใหม่, ญี่ปุ่น |
| `ORG` | `NamedEntityKind::Org` | ปตท, ธนาคารแห่งประเทศไทย, กปปส |

`NamedEntityKind` is defined in `kham-core/src/token.rs` (not `ne.rs`) to avoid circular imports — `ne.rs` imports `Token` from `token.rs`.

## Module Layout

```
kham-core/
├── src/
│   ├── token.rs        # NamedEntityKind enum + TokenKind::Named variant
│   └── ne.rs           # NeTagger struct + impl
└── data/
    └── ne_th.tsv       # built-in gazetteer (hand-curated)
```

## Data File Format (`ne_th.tsv`)

```tsv
# Thai word → NE category (one entry per line)
# Format: <thai_word><TAB><NE_TAG>
# NE_TAG values: PERSON | PLACE | ORG
# Lines starting with # are comments; blank lines ignored
# Duplicate keys: last entry wins

# ── PLACE ──
ไทย	PLACE
กรุงเทพ	PLACE
เชียงใหม่	PLACE

# ── ORG ──
ปตท	ORG
ธนาคารแห่งประเทศไทย	ORG

# ── PERSON ──
ทักษิณ	PERSON
```

Rules:
- Tab-separated, exactly 2 columns
- Thai word must match segmenter output (post-normalize, single token)
- **Multi-token phrases are not supported** — entries that the segmenter splits into 2+ tokens will never match
- Duplicate keys: last entry wins (domain override pattern)
- Sort by category section, then alphabetically within each section

## Gazetteer Coverage (built-in `ne_th.tsv`)

~200 entries organized by section:
- Thai provinces (77)
- Countries (30+)
- World cities (14)
- Thai regions (8)
- Government ministries/bodies (~24)
- State enterprises/companies (~25)
- Universities (8)
- International organizations (~14)
- Royal/political titles (~9)
- Public figures (~6)

## `NamedEntityKind` (in `token.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NamedEntityKind {
    Person,
    Place,
    Org,
}

impl NamedEntityKind {
    pub fn from_tag(s: &str) -> Option<Self>   // "PERSON" / "PLACE" / "ORG"
    pub fn as_tag(self) -> &'static str         // "PERSON" / "PLACE" / "ORG"
    pub fn as_str(self) -> &'static str         // "Person" / "Place" / "Org"
}
```

`NamedEntityKind` is `Copy` — tag lookups return `Option<NamedEntityKind>` by value, not reference.

## `NeTagger` API

```rust
// Built-in gazetteer (embedded via include_str! at compile time)
let tagger = NeTagger::builtin();

// Custom gazetteer (domain-specific entries or test isolation)
let tagger = NeTagger::from_tsv("กิน\tPERSON\n");  // odd, just for tests

// Single-word lookup
tagger.tag("ไทย")      // → Some(NamedEntityKind::Place)
tagger.tag("กิน")      // → None

// Post-processing pass — relabels Thai tokens in-place
let tokens: Vec<Token> = tokenizer.segment(text);
let tagged: Vec<Token> = tagger.tag_tokens(tokens);
// Thai tokens in gazetteer: TokenKind::Thai → TokenKind::Named(kind)
// All other tokens: pass through unchanged

tagger.len()           // → usize (gazetteer entry count)
tagger.is_empty()      // → bool
```

## Integration with FtsTokenizer

`FtsTokenizer` runs NE tagging automatically in `segment_for_fts`:

```rust
// Pipeline: normalize → segment → NE tag → stopword/synonym/ngram
let raw_tokens = self.ne_tagger.tag_tokens(self.tokenizer.segment(&normalized));
```

`FtsToken` carries two NE fields:
- `kind: TokenKind::Named(NamedEntityKind)` — replaces `TokenKind::Thai` for gazetteer hits
- `ne: Option<NamedEntityKind>` — redundant convenience field; `Some(k)` when `kind == Named(k)`, else `None`

POS tagging is skipped for Named tokens (only runs for `TokenKind::Thai`):
```rust
let pos = if token.kind == TokenKind::Thai {
    self.pos_tagger.tag(token.text)
} else {
    None
};
```

Custom gazetteer via builder:
```rust
let fts = FtsTokenizer::builder()
    .ne_tagger(NeTagger::from_tsv("กิน\tPERSON\n"))
    .stopwords(StopwordSet::from_text(""))
    .build();
```

## `TokenKind::Named` in Binding Crates

All four binding crates match on `TokenKind` — `Named` arm must return the entity kind string:

| Crate | Match arm |
|-------|-----------|
| `kham-wasm` | `TokenKind::Named(ne) => ne.as_str()` |
| `kham-python` | `TokenKind::Named(ne) => ne.as_str()` |
| `kham-capi` | `TokenKind::Named(ne) => ne.as_str()` (in `kind_cstring`) |
| `kham-pg` | `TokenKind::Named(_) => 1` (fallback to Thai type; TODO: type 7) |

`ne.as_str()` returns `"Person"`, `"Place"`, or `"Org"`.

## kham-pg Token Type 7 (deferred)

To expose Named entities as a distinct PG token type:
1. Add `TokenKind::Named(_) => 7` in `kham-pg/src/lib.rs`
2. Add `named` entry to `kham_lextypes()` array in `src/shim.c`
3. Add `ALTER TEXT SEARCH CONFIGURATION kham ADD MAPPING FOR named WITH kham_dict` in SQL install script
4. Bump SQL script version, run `make -C kham-pg regress` in Docker to verify

Currently deferred — requires Docker test run.

## Writing Tests

Unit tests live in `kham-core/src/ne.rs`; integration tests in `kham-core/tests/ne.rs`.

```rust
// Single-token NE that the segmenter doesn't split
#[test]
fn place_is_tagged() {
    let t = NeTagger::builtin();
    assert_eq!(t.tag("ไทย"), Some(NamedEntityKind::Place));
}

// Common words must NOT be tagged
#[test]
fn common_word_not_tagged() {
    let t = NeTagger::builtin();
    assert_eq!(t.tag("กิน"), None);
}

// FTS pipeline integration — use single-token NEs only
#[test]
fn fts_token_kind_is_named() {
    let fts = FtsTokenizer::new();
    // "ไทย" segments as one token and is PLACE in gazetteer
    let tokens = fts.segment_for_fts("ไทย");
    let t = tokens.iter().find(|t| t.text == "ไทย").unwrap();
    assert_eq!(t.kind, TokenKind::Named(NamedEntityKind::Place));
    assert_eq!(t.ne, Some(NamedEntityKind::Place));
}
```

**Critical**: Test only entries that are single tokens after segmentation. Multi-token compound names (e.g., กรุงเทพมหานคร → กรุง+เทพ+มหา+นคร) will not match the gazetteer.

## Common Pitfalls

| Pitfall | Fix |
|---------|-----|
| Entry segments into 2+ tokens | Verify with `Tokenizer::new().segment("word")` before adding to TSV |
| Placing `NamedEntityKind` in `ne.rs` | Must be in `token.rs` — circular import otherwise |
| Adding `std` import | Module is `no_std`; use `alloc::collections::BTreeMap`, `alloc::string::String` |
| Testing with กรุงเทพ | Segmenter splits it; use ไทย, เชียงใหม่, or other single-token places |
| Checking POS on Named tokens | POS tagger skips `TokenKind::Named` — `t.pos` is always `None` for NE tokens |

## Expanding the Gazetteer

1. Find single-token Thai NEs (verify with `Tokenizer::new().segment("candidate")`)
2. Add to appropriate section in `kham-core/data/ne_th.tsv`
3. Run `cargo test -p kham-core --test ne` to confirm
4. Run `cargo fmt --all && cargo clippy ...` before committing

## Implementation Checklist

- [x] `NamedEntityKind` enum in `token.rs` with `from_tag`/`as_tag`/`as_str`
- [x] `TokenKind::Named(NamedEntityKind)` variant in `token.rs`
- [x] `NeTagger` struct in `ne.rs` with `builtin`/`from_tsv`/`tag`/`tag_tokens`/`len`/`is_empty`
- [x] `kham-core/data/ne_th.tsv` with ~200 entries
- [x] Register `pub mod ne` in `kham-core/src/lib.rs`
- [x] Re-export `NamedEntityKind` from `kham-core/src/lib.rs`
- [x] `FtsToken.ne: Option<NamedEntityKind>` field
- [x] `FtsTokenizer` wires `ne_tagger` in `segment_for_fts`
- [x] `FtsTokenizerBuilder.ne_tagger()` method
- [x] All 4 binding crates updated with `Named` arm
- [x] Unit tests in `ne.rs`
- [x] Integration tests in `kham-core/tests/ne.rs`
- [ ] kham-pg type-7 SQL update (deferred)
- [ ] NE section in CLAUDE.md
