//! Integration tests for kham-sqlite FTS5 tokenizer extension.
//!
//! Prerequisites:
//!   cargo build -p kham-sqlite          # builds debug dylib
//!
//! Run:
//!   cargo test -p kham-sqlite

use std::path::PathBuf;

use rusqlite::{Connection, LoadExtensionGuard};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve the kham-sqlite dylib path: prefer debug (default test build),
/// fall back to release.
fn ext_path() -> PathBuf {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop(); // workspace root

    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    };

    let debug = root.join(format!("target/debug/libkham_sqlite.{ext}"));
    let release = root.join(format!("target/release/libkham_sqlite.{ext}"));

    if debug.exists() {
        return debug;
    }
    if release.exists() {
        return release;
    }

    // Dylib not present — build it now. cargo test for a cdylib-only crate
    // compiles an rlib for the test harness but does not guarantee the cdylib
    // artifact is written to target/debug/ before integration tests run.
    let cargo = env!("CARGO");
    let status = std::process::Command::new(cargo)
        .args(["build", "-p", "kham-sqlite"])
        .current_dir(&root)
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn `cargo build -p kham-sqlite`: {e}"));
    assert!(
        status.success(),
        "cargo build -p kham-sqlite failed (exit {status})"
    );

    if debug.exists() {
        return debug;
    }
    panic!(
        "kham-sqlite dylib still not found after building.\nLooked in:\n  {debug:?}\n  {release:?}"
    );
}

/// Open an in-memory connection with the kham extension loaded.
fn open_db() -> Connection {
    open_db_tokenize("kham")
}

/// Open an in-memory connection using a custom tokenize string.
///
/// `tokenize` is embedded as-is inside single quotes in SQL, so bare tokenizer
/// arguments (letters, digits, `_`) are safe.  Arguments containing `/`, `.`,
/// or `-` (e.g. file paths) must be single-quoted **inside** the tokenize
/// string itself and then escaped for SQL: pass `"kham synonyms ''/path''"`
/// which becomes the SQL literal `'kham synonyms ''/path'''`.
fn open_db_tokenize(tokenize: &str) -> Connection {
    let conn = Connection::open_in_memory().expect("open :memory:");
    unsafe {
        let _guard = LoadExtensionGuard::new(&conn).expect("load_extension guard");
        conn.load_extension(ext_path(), Some("sqlite3_kham_init"))
            .expect("load kham extension");
    }
    conn.execute_batch(&format!(
        "CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='{tokenize}');"
    ))
    .expect("create fts5 table");
    conn
}

/// Open a db with a synonyms file path properly quoted for the FTS5 tokenize directive.
///
/// FTS5 tokenize barewords allow only `[A-Za-z0-9_]` plus high bytes (>0x7F).
/// File paths must be single-quoted inside the directive so FTS5 accepts them.
/// SQL escaping doubles the inner quotes: `'kham synonyms ''/tmp/s.tsv'''`.
fn open_db_with_synonyms(path: &std::path::Path) -> Connection {
    // Build: tokenize='kham synonyms ''/absolute/path.tsv'''
    // Inner single quotes escape the path arg inside the directive;
    // SQL then escapes those by doubling: '' → literal single quote.
    let path_str = path.display().to_string();
    let sql = format!(
        "CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham synonyms ''{path_str}''');"
    );
    let conn = Connection::open_in_memory().expect("open :memory:");
    unsafe {
        let _guard = LoadExtensionGuard::new(&conn).expect("load_extension guard");
        conn.load_extension(ext_path(), Some("sqlite3_kham_init"))
            .expect("load kham extension");
    }
    conn.execute_batch(&sql).expect("create fts5 table");
    conn
}

/// Insert a single document body.
fn insert(conn: &Connection, body: &str) {
    conn.execute("INSERT INTO docs(body) VALUES (?1)", [body])
        .unwrap();
}

/// Return all matching body strings for a MATCH query.
fn match_bodies(conn: &Connection, query: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT body FROM docs WHERE docs MATCH ?1 ORDER BY rowid")
        .unwrap();
    stmt.query_map([query], |row| row.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

/// Return `true` if the MATCH query finds at least one row.
fn matches(conn: &Connection, query: &str) -> bool {
    !match_bodies(conn, query).is_empty()
}

// ---------------------------------------------------------------------------
// 1. Basic MATCH queries
// ---------------------------------------------------------------------------

#[test]
fn basic_single_word_match() {
    let conn = open_db();
    insert(&conn, "กินข้าวกับปลา");

    assert!(matches(&conn, "ปลา"), "should match ปลา");
    assert!(matches(&conn, "กินข้าว"), "should match compound กินข้าว");
    assert!(!matches(&conn, "วัว"), "วัว is not in document");
}

#[test]
fn basic_multiple_documents() {
    let conn = open_db();
    insert(&conn, "กินข้าวกับปลา");
    insert(&conn, "หมาวิ่งเล่นในสวน");
    insert(&conn, "แมวนอนบนโซฟา");

    assert_eq!(match_bodies(&conn, "กินข้าว").len(), 1);
    assert_eq!(match_bodies(&conn, "หมา").len(), 1);
    assert_eq!(match_bodies(&conn, "แมว").len(), 1);
    assert_eq!(match_bodies(&conn, "สุนัข").len(), 0);
}

#[test]
fn basic_and_query() {
    let conn = open_db();
    insert(&conn, "กินข้าวกับปลา");
    insert(&conn, "กินปลาทอด");

    // Both contain กิน; only first contains ข้าว
    assert_eq!(match_bodies(&conn, "กิน ข้าว").len(), 0); // FTS5 AND by default — no doc has both as separate tokens
                                                        // Actually กินข้าว is a compound — กิน and ข้าว are one token
                                                        // Test AND with truly separate tokens
    insert(&conn, "กิน ปลา วาฬ");
    assert!(matches(&conn, "ปลา"));
}

#[test]
fn basic_or_query() {
    let conn = open_db();
    insert(&conn, "กินข้าวกับปลา");
    insert(&conn, "หมาวิ่งเล่นในสวน");

    let rows = match_bodies(&conn, "ปลา OR หมา");
    assert_eq!(rows.len(), 2);
}

// ---------------------------------------------------------------------------
// 2. RTGS romanization search
// ---------------------------------------------------------------------------

#[test]
fn rtgs_thai_word_findable_by_latin() {
    let conn = open_db();
    // กินปลา segments as กิน | ปลา (กิน is NOT merged into a compound here)
    insert(&conn, "กินปลา");

    // กิน → "kin" (RTGS, confirmed in romanization_th.tsv)
    assert!(matches(&conn, "kin"), "กิน should be findable as 'kin'");

    // ปลา → "pla"
    assert!(matches(&conn, "pla"), "ปลา should be findable as 'pla'");
}

#[test]
fn rtgs_named_entity_romanized() {
    let conn = open_db();
    insert(&conn, "ทักษิณไปกรุงเทพ");

    // กรุงเทพ → "krungthep"
    assert!(
        matches(&conn, "krungthep"),
        "กรุงเทพ should be findable as 'krungthep'"
    );
}

#[test]
fn rtgs_no_false_positives() {
    let conn = open_db();
    insert(&conn, "กินข้าวกับปลา");

    // A romanization form that doesn't match anything in the document
    assert!(!matches(&conn, "mango"), "'mango' should not match");
}

// ---------------------------------------------------------------------------
// 3. lk82 soundex phonetic search
// ---------------------------------------------------------------------------

#[test]
fn soundex_lk82_match_by_code() {
    let conn = open_db();
    // Use กินปลา so both กิน and ปลา are indexed as separate tokens.
    // กินข้าว is a compound that gets a different soundex code.
    insert(&conn, "กินปลา");

    // ปลา → lk82 "4800"
    assert!(matches(&conn, "4800"), "ปลา soundex code 4800 should match");

    // กิน → lk82 "1600"
    assert!(matches(&conn, "1600"), "กิน soundex code 1600 should match");
}

#[test]
fn soundex_none_disables_phonetic() {
    let conn = open_db_tokenize("kham soundex none");
    insert(&conn, "กินข้าวกับปลา");

    // With soundex=none, phonetic codes should NOT match
    assert!(
        !matches(&conn, "4800"),
        "soundex disabled — code 4800 should not match"
    );
    assert!(
        !matches(&conn, "1600"),
        "soundex disabled — code 1600 should not match"
    );

    // Thai token itself still works
    assert!(matches(&conn, "ปลา"), "direct Thai match should still work");
}

#[test]
fn soundex_udom83_enabled() {
    let conn = open_db_tokenize("kham soundex udom83");
    // กินปลา → กิน | ปลา (separate tokens, so RTGS for both is indexed)
    insert(&conn, "กินปลา");

    assert!(
        matches(&conn, "ปลา"),
        "direct match should work with udom83"
    );
    assert!(matches(&conn, "kin"), "RTGS should still work with udom83");
}

// ---------------------------------------------------------------------------
// 4. snippet() and highlight()
// ---------------------------------------------------------------------------

#[test]
fn snippet_highlights_matched_token() {
    let conn = open_db();
    insert(&conn, "กินข้าวกับปลา");

    let mut stmt = conn
        .prepare(
            "SELECT snippet(docs, 0, '<b>', '</b>', '...', 10) \
             FROM docs WHERE docs MATCH 'ปลา'",
        )
        .unwrap();

    let snippet: String = stmt
        .query_map([], |row| row.get(0))
        .unwrap()
        .next()
        .expect("should have a result")
        .unwrap();

    assert!(
        snippet.contains("<b>ปลา</b>"),
        "snippet should highlight ปลา; got: {snippet:?}"
    );
}

#[test]
fn snippet_multiple_matches() {
    let conn = open_db();
    insert(&conn, "กินข้าวกับปลา");
    insert(&conn, "ปลาทอดอร่อยมาก");

    let mut stmt = conn
        .prepare(
            "SELECT snippet(docs, 0, '<b>', '</b>', '...', 10) \
             FROM docs WHERE docs MATCH 'ปลา' ORDER BY rowid",
        )
        .unwrap();

    let snippets: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    assert_eq!(snippets.len(), 2);
    assert!(snippets[0].contains("<b>ปลา</b>"));
    assert!(snippets[1].contains("<b>ปลา</b>"));
}

// ---------------------------------------------------------------------------
// 5. Stopword filtering
// ---------------------------------------------------------------------------

#[test]
fn stopwords_off_by_default_indexes_stopwords() {
    // With default settings, stopwords ARE indexed and queryable.
    let conn = open_db();
    insert(&conn, "กินข้าวกับปลา"); // กับ is a stopword

    assert!(
        matches(&conn, "กับ"),
        "stopwords should be indexed when stopwords=off (default)"
    );
}

#[test]
fn stopwords_on_excludes_stopwords_from_index() {
    let conn = open_db_tokenize("kham stopwords on");
    insert(&conn, "กินข้าวกับปลา"); // กับ is a stopword

    // กับ is a stopword — must NOT be findable
    assert!(
        !matches(&conn, "กับ"),
        "กับ is a stopword and should not be indexed with stopwords=on"
    );

    // นี้ is also a stopword
    insert(&conn, "วันนี้อากาศดี");
    assert!(
        !matches(&conn, "นี้"),
        "นี้ is a stopword and should not be indexed with stopwords=on"
    );
}

#[test]
fn stopwords_on_keeps_non_stopwords() {
    let conn = open_db_tokenize("kham stopwords on");
    insert(&conn, "กินข้าวกับปลา");

    // กิน, ปลา are NOT stopwords — must still be findable
    assert!(matches(&conn, "กินข้าว"), "กินข้าว is not a stopword");
    assert!(matches(&conn, "ปลา"), "ปลา is not a stopword");
}

// ---------------------------------------------------------------------------
// 6. Mixed script — Thai + Latin + Number
// ---------------------------------------------------------------------------

#[test]
fn mixed_script_number_match() {
    let conn = open_db();
    insert(&conn, "ธนาคาร100แห่ง");

    assert!(matches(&conn, "100"), "number token should be indexed");
    assert!(matches(&conn, "ธนาคาร"), "Thai token should be indexed");
}

#[test]
fn mixed_script_latin_match() {
    let conn = open_db();
    insert(&conn, "สวัสดีhelloworld");

    assert!(
        matches(&conn, "helloworld"),
        "Latin token should be indexed"
    );
    assert!(
        matches(&conn, "สวัสดี"),
        "Thai token should be indexed alongside Latin"
    );
}

#[test]
fn mixed_script_all_scripts_independently_queryable() {
    let conn = open_db();
    insert(&conn, "ราคา500บาทต่อkgสำหรับสินค้า");

    assert!(matches(&conn, "500"));
    assert!(matches(&conn, "kg"));
    assert!(matches(&conn, "ราคา"));
    assert!(matches(&conn, "สินค้า"));
}

// ---------------------------------------------------------------------------
// 7. Named entity recognition
// ---------------------------------------------------------------------------

#[test]
fn ne_place_merged_and_queryable() {
    let conn = open_db();
    insert(&conn, "ทักษิณไปกรุงเทพ");

    // กรุงเทพ is a NE Place — NE tagger merges split tokens
    assert!(
        matches(&conn, "กรุงเทพ"),
        "NE place กรุงเทพ should be queryable"
    );
}

#[test]
fn ne_person_queryable() {
    let conn = open_db();
    insert(&conn, "ทักษิณเดินทางไปต่างประเทศ");

    assert!(matches(&conn, "ทักษิณ"), "NE person ทักษิณ should be queryable");
}

// ---------------------------------------------------------------------------
// 8. Configuration options
// ---------------------------------------------------------------------------

#[test]
fn config_ngram_size_zero_is_valid() {
    // Should not crash; unknown tokens are still indexed as-is.
    let conn = open_db_tokenize("kham ngram_size 0");
    insert(&conn, "กินข้าวกับปลา");
    assert!(matches(&conn, "ปลา"));
}

#[test]
fn config_ngram_size_two_is_valid() {
    let conn = open_db_tokenize("kham ngram_size 2");
    insert(&conn, "กินข้าวกับปลา");
    assert!(matches(&conn, "ปลา"));
}

#[test]
fn config_all_options_combined() {
    let conn = open_db_tokenize("kham soundex udom83 stopwords on ngram_size 2");
    insert(&conn, "กินข้าวกับปลา");

    assert!(matches(&conn, "ปลา"), "basic match");
    assert!(!matches(&conn, "กับ"), "กับ is a stopword");
}

// ---------------------------------------------------------------------------
// 9. Custom synonym map (synonyms=<path>)
// ---------------------------------------------------------------------------

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn custom_synonyms_loaded_from_file() {
    let syn_path = fixtures_dir().join("synonyms.tsv");
    let conn = open_db_with_synonyms(&syn_path);

    // กินปลา → กิน | ปลา; synonyms.tsv maps กิน → ทาน, รับประทาน
    insert(&conn, "กินปลา");

    assert!(
        matches(&conn, "ทาน"),
        "synonym 'ทาน' should find doc containing กิน"
    );
    assert!(
        matches(&conn, "รับประทาน"),
        "synonym 'รับประทาน' should find doc containing กิน"
    );
}

#[test]
fn custom_synonyms_primary_token_still_works() {
    let syn_path = fixtures_dir().join("synonyms.tsv");
    let conn = open_db_with_synonyms(&syn_path);
    insert(&conn, "กินปลา");

    assert!(
        matches(&conn, "กิน"),
        "primary token กิน must still match even with synonyms loaded"
    );
}

#[test]
fn custom_synonyms_secondary_entry() {
    let syn_path = fixtures_dir().join("synonyms.tsv");
    let conn = open_db_with_synonyms(&syn_path);
    insert(&conn, "อยู่ในบ้านหลังใหญ่");

    // synonyms.tsv maps บ้าน → เรือน
    assert!(
        matches(&conn, "เรือน"),
        "synonym 'เรือน' should find doc containing บ้าน"
    );
    assert!(matches(&conn, "บ้าน"), "primary token บ้าน must still match");
}

#[test]
fn custom_synonyms_missing_file_returns_error() {
    let bad_path = fixtures_dir().join("nonexistent_synonyms.tsv");

    let conn = Connection::open_in_memory().expect("open :memory:");
    unsafe {
        let _guard = LoadExtensionGuard::new(&conn).expect("load_extension guard");
        conn.load_extension(ext_path(), Some("sqlite3_kham_init"))
            .expect("load kham extension");
    }

    // Path must be single-quoted inside the directive so FTS5 parses it
    let path_str = bad_path.display().to_string();
    let result = conn.execute_batch(&format!(
        "CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham synonyms ''{path_str}''');"
    ));
    assert!(
        result.is_err(),
        "missing synonyms file should cause table creation to fail"
    );
}

// ---------------------------------------------------------------------------
// 10. Custom dictionary (dict=<path>)
// ---------------------------------------------------------------------------

/// Open a db with a custom word list file properly quoted for the FTS5 tokenize directive.
fn open_db_with_dict(path: &std::path::Path) -> Connection {
    let path_str = path.display().to_string();
    let sql =
        format!("CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham dict ''{path_str}''');");
    let conn = Connection::open_in_memory().expect("open :memory:");
    unsafe {
        let _guard = LoadExtensionGuard::new(&conn).expect("load_extension guard");
        conn.load_extension(ext_path(), Some("sqlite3_kham_init"))
            .expect("load kham extension");
    }
    conn.execute_batch(&sql).expect("create fts5 table");
    conn
}

#[test]
fn custom_dict_domain_word_is_indexed() {
    let dict_path = fixtures_dir().join("custom_words.txt");
    let conn = open_db_with_dict(&dict_path);

    // โปรแกรมเมอร์ is in custom_words.txt but not in the built-in dictionary
    // With the custom dict loaded it should be recognized as a single Thai token.
    insert(&conn, "โปรแกรมเมอร์ไทย");
    assert!(
        matches(&conn, "โปรแกรมเมอร์"),
        "custom word โปรแกรมเมอร์ should be recognized and indexed"
    );
}

#[test]
fn custom_dict_builtin_words_still_work() {
    let dict_path = fixtures_dir().join("custom_words.txt");
    let conn = open_db_with_dict(&dict_path);

    // Built-in words must remain functional
    insert(&conn, "กินข้าวกับปลา");
    assert!(
        matches(&conn, "ปลา"),
        "built-in word ปลา should still match"
    );
    assert!(
        matches(&conn, "กินข้าว"),
        "built-in compound กินข้าว should still match"
    );
}

#[test]
fn custom_dict_missing_file_returns_error() {
    let bad_path = fixtures_dir().join("nonexistent_words.txt");

    let conn = Connection::open_in_memory().expect("open :memory:");
    unsafe {
        let _guard = LoadExtensionGuard::new(&conn).expect("load_extension guard");
        conn.load_extension(ext_path(), Some("sqlite3_kham_init"))
            .expect("load kham extension");
    }

    let path_str = bad_path.display().to_string();
    let result = conn.execute_batch(&format!(
        "CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham dict ''{path_str}''');"
    ));
    assert!(
        result.is_err(),
        "missing dict file should cause table creation to fail"
    );
}

#[test]
fn custom_dict_combined_with_synonyms() {
    let dict_path = fixtures_dir().join("custom_words.txt");
    let syn_path = fixtures_dir().join("synonyms.tsv");

    let dict_str = dict_path.display().to_string();
    let syn_str = syn_path.display().to_string();
    let sql = format!(
        "CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham dict ''{dict_str}'' synonyms ''{syn_str}''');"
    );

    let conn = Connection::open_in_memory().expect("open :memory:");
    unsafe {
        let _guard = LoadExtensionGuard::new(&conn).expect("load_extension guard");
        conn.load_extension(ext_path(), Some("sqlite3_kham_init"))
            .expect("load kham extension");
    }
    conn.execute_batch(&sql).expect("create fts5 table");

    insert(&conn, "กินโปรแกรมเมอร์");
    assert!(
        matches(&conn, "โปรแกรมเมอร์"),
        "custom word should be indexed"
    );
    // synonyms.tsv maps กิน → ทาน
    assert!(
        matches(&conn, "ทาน"),
        "synonyms should still work alongside custom dict"
    );
}
