use crate::corpus::CorpusDb;
use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

#[derive(Serialize)]
pub struct KwicLine {
    pub doc_id: i64,
    pub filename: String,
    pub left: Vec<String>,
    pub node: String,
    pub right: Vec<String>,
}

pub fn search(
    db: &CorpusDb,
    word: &str,
    context: usize,
    limit: usize,
    offset: usize,
) -> Result<Vec<KwicLine>> {
    let mut stmt = db.conn.prepare_cached(
        "SELECT t.doc_id, d.filename, t.pos
         FROM tokens t
         JOIN docs d ON d.doc_id = t.doc_id
         WHERE t.word = ?1
         LIMIT ?2 OFFSET ?3",
    )?;

    let hits: Vec<(i64, String, i64)> = stmt
        .query_map(params![word, limit as i64, offset as i64], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut lines = Vec::with_capacity(hits.len());

    for (doc_id, filename, node_pos) in hits {
        let left_start = (node_pos - context as i64).max(0);
        let right_end = node_pos + context as i64 + 1;

        let mut ctx_stmt = db.conn.prepare_cached(
            "SELECT pos, word FROM tokens
             WHERE doc_id = ?1 AND pos >= ?2 AND pos < ?3
             ORDER BY pos",
        )?;

        let ctx: Vec<(i64, String)> = ctx_stmt
            .query_map(params![doc_id, left_start, right_end], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<rusqlite::Result<_>>()?;

        let mut left = Vec::new();
        let mut node_word = word.to_string();
        let mut right = Vec::new();

        for (pos, w) in ctx {
            if pos < node_pos {
                left.push(w);
            } else if pos == node_pos {
                node_word = w;
            } else {
                right.push(w);
            }
        }

        lines.push(KwicLine {
            doc_id,
            filename,
            left,
            node: node_word,
            right,
        });
    }

    Ok(lines)
}
