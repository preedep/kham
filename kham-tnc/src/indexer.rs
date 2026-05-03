use crate::corpus::CorpusDb;
use anyhow::Result;
use kham_core::fts::FtsTokenizer;
use kham_core::token::TokenKind;

struct OwnedTokenRow {
    pos: usize,
    word: String,
    pos_tag: Option<&'static str>,
    ne_tag: Option<&'static str>,
}

pub fn index_file(
    db: &CorpusDb,
    path: &str,
    genre: Option<&str>,
    domain: Option<&str>,
) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    let fts = FtsTokenizer::new();
    let tokens = fts.segment_for_fts(&text);

    let char_count = text.chars().count();
    let rows: Vec<OwnedTokenRow> = tokens
        .into_iter()
        .filter(|t| !matches!(t.kind, TokenKind::Whitespace))
        .enumerate()
        .map(|(pos, t)| OwnedTokenRow {
            pos,
            word: t.text,
            pos_tag: t.pos.map(|p| p.as_tag()),
            ne_tag: t.ne.map(|n| n.as_tag()),
        })
        .collect();

    let token_count = rows.len();
    let doc_id = db.insert_doc(path, genre, domain, char_count, token_count)?;

    {
        use crate::corpus::TokenRow;
        let token_rows: Vec<TokenRow> = rows
            .iter()
            .map(|r| TokenRow {
                doc_id,
                pos: r.pos,
                word: &r.word,
                pos_tag: r.pos_tag,
                ne_tag: r.ne_tag,
                char_start: 0,
                char_end: 0,
            })
            .collect();
        db.insert_tokens(&token_rows)?;
    }

    tracing::info!(path, doc_id, token_count, "indexed");
    Ok(())
}
