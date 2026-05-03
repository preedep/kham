# kham-tnc

Professional Thai corpus analysis web tool — KWIC, collocation, frequency, POS/NE search. Built on `kham-core` + axum + SQLite + HTMX.

## Architecture

```
kham-tnc/
├── src/
│   ├── main.rs       # CLI entry (Serve / Index subcommands)
│   ├── corpus.rs     # CorpusDb: SQLite schema, insert helpers, stats
│   ├── indexer.rs    # index_file(): segment → FTS tag → write tokens table
│   ├── kwic.rs       # KWIC concordance search
│   ├── freq.rs       # Word frequency list with POS/NE filter
│   ├── collocate.rs  # Collocation stats: MI, logDice, t-score, log-likelihood
│   └── api.rs        # axum REST handlers + serve()
└── static/           # HTMX + Tailwind frontend (future)
```

## SQLite Schema

```sql
docs   (doc_id, filename, genre, domain, char_count, token_count, created_at)
tokens (id, doc_id, pos, word, pos_tag, ne_tag, char_start, char_end)
```

Indexes on `tokens.word`, `tokens.doc_id`, `tokens.pos_tag`, `tokens.ne_tag`.

## REST API

| Endpoint          | Query params                                             |
|-------------------|----------------------------------------------------------|
| `GET /api/stats`     | —                                                     |
| `GET /api/kwic`      | `word`, `context=5`, `limit=50`, `offset=0`           |
| `GET /api/freq`      | `pos`, `ne`, `min_freq=2`, `limit=50`, `offset=0`     |
| `GET /api/collocate` | `word`, `left=5`, `right=5`, `min_freq=2`             |

## Commands

```bash
# Index a file into a corpus
cargo run -p kham-tnc -- index myfile.txt --corpus corpus.sqlite --genre news

# Start the web server
cargo run -p kham-tnc -- serve --corpus corpus.sqlite --port 8080
```

## Threading note

`rusqlite::Connection` is not `Sync`. `AppState` wraps `CorpusDb` in `Mutex<CorpusDb>`. All handlers lock before use. For high-concurrency production use, replace with a connection pool (e.g. `r2d2-sqlite`).

## Phase status

- [x] Phase 1 skeleton: corpus schema, indexer, KWIC, frequency, collocation, REST API
- [ ] Phase 1 complete: wildcard search, sort options, CSV export, HTMX UI
- [ ] Phase 2: POS-aware query syntax, n-gram analysis, dispersion, visualizations
- [ ] Phase 3: multi-corpus comparison, keyword analysis, deployment artifacts
