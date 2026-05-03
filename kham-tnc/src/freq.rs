use crate::corpus::CorpusDb;
use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

#[derive(Serialize)]
pub struct UntaggedEntry {
    pub word: String,
    pub count: i64,
    pub docs: i64,
    pub per_million: f64,
}

pub fn untagged_words(db: &CorpusDb, limit: usize, offset: usize) -> Result<Vec<UntaggedEntry>> {
    let total_tokens: i64 =
        db.conn
            .query_row("SELECT COALESCE(SUM(token_count),0) FROM docs", [], |r| {
                r.get(0)
            })?;

    let mut stmt = db.conn.prepare(
        "SELECT word, COUNT(*) AS cnt, COUNT(DISTINCT doc_id) AS dc
         FROM tokens
         WHERE pos_tag IS NULL AND ne_tag IS NULL AND length(word) >= 2
         GROUP BY word
         ORDER BY cnt DESC
         LIMIT ?1 OFFSET ?2",
    )?;

    let per_million_base = if total_tokens > 0 {
        1_000_000.0 / total_tokens as f64
    } else {
        0.0
    };

    let rows = stmt
        .query_map(params![limit as i64, offset as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(rows
        .into_iter()
        .map(|(word, count, docs)| UntaggedEntry {
            per_million: count as f64 * per_million_base,
            word,
            count,
            docs,
        })
        .collect())
}

#[derive(Serialize)]
pub struct FreqEntry {
    pub word: String,
    pub count: i64,
    pub docs: i64,
    pub per_million: f64,
    pub pos_tag: Option<String>,
    pub ne_tag: Option<String>,
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

    type FreqRow = (String, i64, i64, Option<String>, Option<String>);
    // Each arm selects (word, cnt, dc, pos_tag, ne_tag).
    // SQLite GROUP BY returns any row's pos_tag/ne_tag for the group — sufficient for display.
    let rows: Vec<FreqRow> = match (pos_filter, ne_filter) {
        (Some(pos), Some(ne)) => {
            let mut stmt = db.conn.prepare(
                "SELECT word, COUNT(*) AS cnt, COUNT(DISTINCT doc_id) AS dc,
                            pos_tag, ne_tag
                     FROM tokens WHERE pos_tag = ?1 AND ne_tag = ?2
                     GROUP BY word HAVING cnt >= ?3
                     ORDER BY cnt DESC LIMIT ?4 OFFSET ?5",
            )?;
            let r = stmt
                .query_map(params![pos, ne, min_freq, lim, off], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
            r
        }
        (Some(pos), None) => {
            let mut stmt = db.conn.prepare(
                "SELECT word, COUNT(*) AS cnt, COUNT(DISTINCT doc_id) AS dc,
                            pos_tag, ne_tag
                     FROM tokens WHERE pos_tag = ?1
                     GROUP BY word HAVING cnt >= ?2
                     ORDER BY cnt DESC LIMIT ?3 OFFSET ?4",
            )?;
            let r = stmt
                .query_map(params![pos, min_freq, lim, off], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
            r
        }
        (None, Some(ne)) => {
            let mut stmt = db.conn.prepare(
                "SELECT word, COUNT(*) AS cnt, COUNT(DISTINCT doc_id) AS dc,
                            pos_tag, ne_tag
                     FROM tokens WHERE ne_tag = ?1
                     GROUP BY word HAVING cnt >= ?2
                     ORDER BY cnt DESC LIMIT ?3 OFFSET ?4",
            )?;
            let r = stmt
                .query_map(params![ne, min_freq, lim, off], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
            r
        }
        (None, None) => {
            let mut stmt = db.conn.prepare(
                "SELECT word, COUNT(*) AS cnt, COUNT(DISTINCT doc_id) AS dc,
                            pos_tag, ne_tag
                     FROM tokens
                     GROUP BY word HAVING cnt >= ?1
                     ORDER BY cnt DESC LIMIT ?2 OFFSET ?3",
            )?;
            let r = stmt
                .query_map(params![min_freq, lim, off], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
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
        .map(|(word, count, docs, pos_tag, ne_tag)| FreqEntry {
            per_million: count as f64 * per_million_base,
            word,
            count,
            docs,
            pos_tag,
            ne_tag,
        })
        .collect())
}
