# kham-web Roadmap

Website for **kham.io** — built with Astro + Tailwind + kham-wasm. Lives at `kham-web/` in the monorepo.

---

## Phase 1 — Foundation (v0.1)

Goal: scaffolded project, CI pipeline, and a deployable landing page with badges.

- [ ] Scaffold `kham-web/` with Astro + Tailwind (`npm create astro`)
- [ ] Configure `astro.config.mjs` — `output: 'static'`, base URL `kham.io`
- [ ] `NavBar.astro` + `Footer.astro` components
- [ ] `BadgeRow.astro` — crates.io version, docs.rs, CI status, license, MSRV, downloads
- [ ] Landing page (`/`) — hero headline, one-paragraph pitch, badge row, CTA buttons
- [ ] GitHub Actions workflow `.github/workflows/web.yml` — build + deploy to GitHub Pages
- [ ] Domain setup — `kham.io` → GitHub Pages / Cloudflare Pages

---

## Phase 2 — Live Demo (v0.2)

Goal: interactive Thai segmentation playground powered by kham-wasm in the browser.

- [x] Build kham-wasm and wire into Astro via `public/wasm/` (copied by `setup:wasm` script)
- [x] `LiveDemo.astro` component — Thai text input → tokenized output table
- [x] Token table columns: `text`, `kind`, `char span`, `byte span` (POS/NE/romanization deferred to v0.7 — see Phase 7)
- [ ] Toggle options: FTS mode, soundex algorithm (lk82 / udom83 / MetaSound) — deferred to v0.7 (see Phase 7)
- [x] `/demo` full-page playground with sample texts
- [x] Lazy-load WASM (dynamic import via `is:inline` script) — page not blocked
- [x] Embed demo teaser on landing page (`/`)

---

## Phase 3 — Getting Started & Integrations (v0.3)

Goal: installation guides for every target so new users can be productive in minutes.

- [x] `/getting-started` — quickstart sections per target:
  - Rust (Cargo.toml snippet + minimal example)
  - Python (`pip install kham` + `segment()` example)
  - WASM / npm (browser + Node.js)
  - CLI (`cargo install kham-cli` + usage)
  - PostgreSQL FTS5 (extension install + SQL example)
  - SQLite FTS5 (`.load` + FTS5 virtual table example)
- [x] `CodeBlock.astro` — multi-language tabbed code snippets with copy button (uses Astro `<Code>` + Shiki, no extra packages)
- [x] `/integrations/postgresql` — full PG setup guide + ts_headline example
- [x] `/integrations/sqlite` — full SQLite FTS5 setup guide
- [x] `/integrations/python` — PyO3 binding guide + token fields
- [x] `/integrations/wasm` — browser + Node.js guide

---

## Phase 4 — API Reference & Modules (v0.4)

Goal: developer-facing API docs for kham-core public API.

- [x] `/api` — overview table of all public modules with docs.rs links
- [x] Document each module with Thai + English examples:
  - `Tokenizer` / `Token` / `TokenKind` (note: public type is `Tokenizer`, not `Segmenter`)
  - `FtsTokenizer` + full builder pipeline options
  - `PosTagger` / `PosTag` (13 ORCHID-derived categories, tabulated)
  - `NeTagger` / `NamedEntityKind` (Person / Place / Org)
  - `RomanizationMap` (RTGS)
  - `number` module: digit conversion, word parsing, baht text
  - `sentence` / `split_sentences`
  - `soundex` (lk82 / udom83 / MetaSound / cross-language)
- [x] Link each module to docs.rs page

---

## Phase 5 — Benchmarks & Changelog (v0.5)

Goal: transparency on performance and release history.

- [x] `/benchmarks` — criterion throughput numbers vs nlpO3 / PyThaiNLP
- [x] `/benchmarks` — accuracy F1 table against CC0 gold corpus
- [x] `/changelog` — per-version release notes (pulled from git tags / CHANGELOG.md)
- [x] CHANGELOG.md at repo root (keep in sync with releases)

---

## Phase 6 — Polish & SEO (v0.6)

Goal: production-ready site, discoverable by search engines and Thai NLP community.

- [x] `/license` — full MIT + Apache-2.0 text, corpus attribution table (TNC, CC-BY, CC-BY-SA)
- [x] Open Graph / Twitter card meta tags on all pages
- [x] `sitemap.xml` + `robots.txt` (Astro sitemap integration via @astrojs/sitemap)
- [x] Thai + English page titles and descriptions
- [x] Dark mode support (Tailwind `dark:` classes on NavBar, Footer, Layout, LiveDemo, license page)
- [x] Accessibility audit (skip-to-content link, aria-live, aria-label, aria-busy, scope on th, role on nav)
- [ ] Lighthouse score ≥ 90 on all pages (verify after deployment)

---

## Phase 7 — WASM API Expansion & Enhanced Demo (v0.7)

Goal: expose the full kham-core NLP pipeline in WASM and upgrade the live demo to show POS, NE, romanization, and more.

### WASM bindings (`kham-wasm/src/lib.rs`)

- [ ] `segment_fts(text)` — returns `FtsToken[]` with `text`, `kind`, `pos`, `ne`, `is_stop`, `synonyms` fields (wraps `FtsTokenizer::segment_for_fts`)
- [ ] `romanize(text)` — returns `{word, roman}[]` via `RomanizationMap::romanize_tokens`
- [ ] `split_sentences(text)` — returns `string[]` sentence boundaries via `split_sentences()`
- [ ] `soundex_word(word, algo)` — returns soundex code string; `algo` = `"lk82"` | `"udom83"` | `"metasound"` | `"cross"`
- [ ] `normalize(text)` — returns normalized string via `normalizer::normalize`

### Demo UI (`kham-web/`)

- [ ] FTS mode toggle — switch `segment_tokens` → `segment_fts`; show `pos`, `ne`, `is_stop` columns
- [ ] POS badge column — colour-coded POS tag per Thai token (e.g. NOUN=blue, VERB=green, ADJ=yellow)
- [ ] NE highlight — Person / Place / Org tokens highlighted with distinct colours
- [ ] Romanization column — RTGS roman for each Thai token (toggle on/off)
- [ ] Soundex column — lk82 / udom83 / MetaSound selector; show code next to each Thai token
- [ ] Sentence splitter panel — paste paragraph → show sentence boundaries
- [ ] Normalizer panel — show raw vs normalised diff

---

## Deferred / Future

- Search (Pagefind or Algolia DocSearch)
- i18n — Thai-language version of docs
- Versioned docs (v0.4 / v0.5 side by side)
- Blog / announcements section
- Embed GitHub star count widget
