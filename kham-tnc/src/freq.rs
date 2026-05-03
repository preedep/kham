use crate::corpus::CorpusDb;
use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

#[derive(Serialize)]
pub struct FreqEntry {
    pub word: String,
    pub count: i64,
    pub docs: i64,
    pub per_million: f64,
}

pub fn word_frequency(
    db: &CorpusDb,
    pos_filter: Option<&str>,
    ne_filter: Option<&str>,
    min_freq: i64,
    limit: usize,
    offset: usize,
) -> Result<Vec<FreqEntry>> {
    let total_tokens: i64 =
        db.conn
            .query_row("SELECT COALESCE(SUM(token_count),0) FROM docs", [], |r| {
                r.get(0)
            })?;

    let lim = limit as i64;
    let off = offset as i64;

    let rows: Vec<(String, i64, i64)> = match (pos_filter, ne_filter) {
        (Some(pos), Some(ne)) => {
            let mut stmt = db.conn.prepare(
                "SELECT word, COUNT(*) AS cnt, COUNT(DISTINCT doc_id) AS dc
                 FROM tokens WHERE pos_tag = ?1 AND ne_tag = ?2
                 GROUP BY word HAVING cnt >= ?3
                 ORDER BY cnt DESC LIMIT ?4 OFFSET ?5",
            )?;
            let r = stmt
                .query_map(params![pos, ne, min_freq, lim, off], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
            r
        }
        (Some(pos), None) => {
            let mut stmt = db.conn.prepare(
                "SELECT word, COUNT(*) AS cnt, COUNT(DISTINCT doc_id) AS dc
                 FROM tokens WHERE pos_tag = ?1
                 GROUP BY word HAVING cnt >= ?2
                 ORDER BY cnt DESC LIMIT ?3 OFFSET ?4",
            )?;
            let r = stmt
                .query_map(params![pos, min_freq, lim, off], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
            r
        }
        (None, Some(ne)) => {
            let mut stmt = db.conn.prepare(
                "SELECT word, COUNT(*) AS cnt, COUNT(DISTINCT doc_id) AS dc
                 FROM tokens WHERE ne_tag = ?1
                 GROUP BY word HAVING cnt >= ?2
                 ORDER BY cnt DESC LIMIT ?3 OFFSET ?4",
            )?;
            let r = stmt
                .query_map(params![ne, min_freq, lim, off], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
            r
        }
        (None, None) => {
            let mut stmt = db.conn.prepare(
                "SELECT word, COUNT(*) AS cnt, COUNT(DISTINCT doc_id) AS dc
                 FROM tokens
                 GROUP BY word HAVING cnt >= ?1
                 ORDER BY cnt DESC LIMIT ?2 OFFSET ?3",
            )?;
            let r = stmt
                .query_map(params![min_freq, lim, off], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
            r
        }
    };

    let per_million_base = if total_tokens > 0 {
        1_000_000.0 / total_tokens as f64
    } else {
        0.0
    };

    Ok(rows
        .into_iter()
        .map(|(word, count, docs)| FreqEntry {
            per_million: count as f64 * per_million_base,
            word,
            count,
            docs,
        })
        .collect())
}
