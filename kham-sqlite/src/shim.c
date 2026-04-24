/*
 * shim.c — C helpers for the kham SQLite FTS5 tokenizer extension.
 *
 * SQLITE_EXTENSION_INIT1/2 set up the sqlite3_api vtable so that SQLite
 * API functions (sqlite3_prepare, sqlite3_bind_pointer, etc.) called from
 * this file go through the vtable provided at extension-load time rather
 * than linking directly against libsqlite3.
 *
 * The public entry point sqlite3_kham_init / sqlite3_khamsqlite_init is
 * defined in lib.rs as #[no_mangle] so it is guaranteed to appear in the
 * dylib symbol table.  It calls the two helpers below.
 */

#include <stddef.h>
#include "sqlite3ext.h"
SQLITE_EXTENSION_INIT1

/*
 * kham_sqlite_setup_api — store the SQLite API vtable for this extension.
 * Must be called at the start of the extension entry point before any
 * other SQLite API calls.
 */
void
kham_sqlite_setup_api(const sqlite3_api_routines *pApi)
{
    SQLITE_EXTENSION_INIT2(pApi);
}

/*
 * kham_sqlite_get_fts5api — retrieve the fts5_api pointer for db using the
 * bind-pointer trick documented in the SQLite FTS5 tokenizer API reference.
 * Returns NULL if FTS5 is not available in this SQLite build.
 */
void *
kham_sqlite_get_fts5api(sqlite3 *db)
{
    void         *pRet  = NULL;
    sqlite3_stmt *pStmt = NULL;

    if (SQLITE_OK == sqlite3_prepare(db, "SELECT fts5(?1)", -1, &pStmt, NULL)) {
        sqlite3_bind_pointer(pStmt, 1, &pRet, "fts5_api_ptr", NULL);
        sqlite3_step(pStmt);
    }
    sqlite3_finalize(pStmt);
    return pRet;
}
