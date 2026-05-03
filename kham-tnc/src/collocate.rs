use crate::corpus::CorpusDb;
use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

#[derive(Serialize)]
pub struct Collocate {
    pub word: String,
    pub freq: i64,
    pub mi: f64,
    pub log_dice: f64,
    pub t_score: f64,
    pub log_likelihood: f64,
}

pub fn collocates(
    db: &CorpusDb,
    node: &str,
    left_span: usize,
    right_span: usize,
    min_freq: i64,
) -> Result<Vec<Collocate>> {
    let total_tokens: i64 =
        db.conn
            .query_row("SELECT COALESCE(SUM(token_count),0) FROM docs", [], |r| {
                r.get(0)
            })?;

    let node_freq: i64 = db.conn.query_row(
        "SELECT COUNT(*) FROM tokens WHERE word = ?1",
        params![node],
        |r| r.get(0),
    )?;

    let span = left_span as i64;
    let rspan = right_span as i64;

    let mut stmt = db.conn.prepare(
        "SELECT t2.word, COUNT(*) AS cofreq
         FROM tokens t1
         JOIN tokens t2 ON t2.doc_id = t1.doc_id
             AND t2.pos >= t1.pos - ?1
             AND t2.pos <= t1.pos + ?2
             AND t2.pos <> t1.pos
         WHERE t1.word = ?3
         GROUP BY t2.word
         HAVING cofreq >= ?4
         ORDER BY cofreq DESC",
    )?;

    let hits: Vec<(String, i64)> = stmt
        .query_map(params![span, rspan, node, min_freq], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let n = total_tokens as f64;

    let mut results = Vec::with_capacity(hits.len());
    for (word, cofreq) in hits {
        let collocate_freq: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM tokens WHERE word = ?1",
            params![word],
            |r| r.get(0),
        )?;

        let o11 = cofreq as f64;
        let r1 = node_freq as f64;
        let c1 = collocate_freq as f64;
        let expected = r1 * c1 / n;

        let mi = if expected > 0.0 {
            (o11 / expected).log2()
        } else {
            0.0
        };
        let log_dice = if r1 + c1 > 0.0 {
            (2.0 * o11 / (r1 + c1)).log2() + 14.0
        } else {
            0.0
        };
        let t_score = if expected > 0.0 {
            (o11 - expected) / o11.sqrt()
        } else {
            0.0
        };
        let ll = if o11 > 0.0 && expected > 0.0 {
            2.0 * o11 * (o11 / expected).ln()
        } else {
            0.0
        };

        results.push(Collocate {
            word,
            freq: cofreq,
            mi,
            log_dice,
            t_score,
            log_likelihood: ll,
        });
    }

    results.sort_by(|a, b| {
        b.log_dice
            .partial_cmp(&a.log_dice)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(results)
}
