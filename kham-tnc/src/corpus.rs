use anyhow::Result;
use rusqlite::{params, Connection};

pub struct CorpusDb {
    pub conn: Connection,
}

impl CorpusDb {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS docs (
                doc_id      INTEGER PRIMARY KEY,
                filename    TEXT NOT NULL,
                genre       TEXT,
                domain      TEXT,
                char_count  INTEGER NOT NULL DEFAULT 0,
                token_count INTEGER NOT NULL DEFAULT 0,
                created_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS tokens (
                id         INTEGER PRIMARY KEY,
                doc_id     INTEGER NOT NULL REFERENCES docs(doc_id),
                pos        INTEGER NOT NULL,
                word       TEXT NOT NULL,
                pos_tag    TEXT,
                ne_tag     TEXT,
                char_start INTEGER NOT NULL,
                char_end   INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_tokens_word    ON tokens(word);
            CREATE INDEX IF NOT EXISTS idx_tokens_doc_id  ON tokens(doc_id);
            CREATE INDEX IF NOT EXISTS idx_tokens_pos_tag ON tokens(pos_tag);
            CREATE INDEX IF NOT EXISTS idx_tokens_ne_tag  ON tokens(ne_tag);

            CREATE TABLE IF NOT EXISTS corrections (
                word        TEXT NOT NULL PRIMARY KEY,
                correct_pos TEXT,
                correct_ne  TEXT,
                note        TEXT,
                created_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );
        ",
        )?;
        Ok(())
    }

    pub fn insert_doc(
        &self,
        filename: &str,
        genre: Option<&str>,
        domain: Option<&str>,
        char_count: usize,
        token_count: usize,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO docs (filename, genre, domain, char_count, token_count)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                filename,
                genre,
                domain,
                char_count as i64,
                token_count as i64
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn insert_tokens(&self, rows: &[TokenRow]) -> Result<()> {
        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO tokens (doc_id, pos, word, pos_tag, ne_tag, char_start, char_end)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for r in rows {
            stmt.execute(params![
                r.doc_id,
                r.pos as i64,
                r.word,
                r.pos_tag,
                r.ne_tag,
                r.char_start as i64,
                r.char_end as i64,
            ])?;
        }
        Ok(())
    }

    pub fn save_correction(
        &self,
        word: &str,
        correct_pos: Option<&str>,
        correct_ne: Option<&str>,
        note: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO corrections (word, correct_pos, correct_ne, note)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(word) DO UPDATE SET
               correct_pos = excluded.correct_pos,
               correct_ne  = excluded.correct_ne,
               note        = excluded.note,
               created_at  = datetime('now')",
            params![word, correct_pos, correct_ne, note],
        )?;
        Ok(())
    }

    pub fn delete_correction(&self, word: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM corrections WHERE word = ?1", params![word])?;
        Ok(())
    }

    pub fn list_corrections(&self, limit: usize, offset: usize) -> Result<Vec<Correction>> {
        let mut stmt = self.conn.prepare(
            "SELECT word, correct_pos, correct_ne, note, created_at
             FROM corrections
             ORDER BY created_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit as i64, offset as i64], |r| {
            Ok(Correction {
                word: r.get(0)?,
                correct_pos: r.get(1)?,
                correct_ne: r.get(2)?,
                note: r.get(3)?,
                created_at: r.get(4)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn export_corrections_pos(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT word, correct_pos FROM corrections
             WHERE correct_pos IS NOT NULL AND correct_pos != ''
             ORDER BY word",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn export_corrections_ne(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT word, correct_ne FROM corrections
             WHERE correct_ne IS NOT NULL AND correct_ne != ''
             ORDER BY word",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn corpus_stats(&self) -> Result<CorpusStats> {
        let doc_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM docs", [], |r| r.get(0))?;
        let token_count: i64 =
            self.conn
                .query_row("SELECT COALESCE(SUM(token_count),0) FROM docs", [], |r| {
                    r.get(0)
                })?;
        let type_count: i64 =
            self.conn
                .query_row("SELECT COUNT(DISTINCT word) FROM tokens", [], |r| r.get(0))?;
        Ok(CorpusStats {
            doc_count,
            token_count,
            type_count,
        })
    }
}

pub struct TokenRow<'a> {
    pub doc_id: i64,
    pub pos: usize,
    pub word: &'a str,
    pub pos_tag: Option<&'a str>,
    pub ne_tag: Option<&'a str>,
    pub char_start: usize,
    pub char_end: usize,
}

#[derive(serde::Serialize)]
pub struct CorpusStats {
    pub doc_count: i64,
    pub token_count: i64,
    pub type_count: i64,
}

#[derive(serde::Serialize)]
pub struct Correction {
    pub word: String,
    pub correct_pos: Option<String>,
    pub correct_ne: Option<String>,
    pub note: Option<String>,
    pub created_at: String,
}
