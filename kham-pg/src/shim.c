/*
 * shim.c — PostgreSQL fmgr helpers for the kham text-search parser.
 *
 * Public symbols (Pg_magic_func, kham_start, kham_gettoken, kham_end,
 * kham_lextypes, pg_finfo_*) are defined in lib.rs as #[no_mangle] Rust
 * functions so they are guaranteed to appear in the dynamic symbol table.
 * This file provides the C implementations called by those trampolines.
 */

#include "postgres.h"
#include "fmgr.h"
#include "tsearch/ts_public.h"
#include "utils/palloc.h"

/* ----------------------------------------------------------------
 * Module magic data — must be compiled against the target PG headers
 * so all version fields are correct for the running server.
 * Called from the Rust Pg_magic_func() trampoline in lib.rs.
 * ---------------------------------------------------------------- */
const Pg_magic_struct *
kham_pg_magic_impl(void)
{
    static const Pg_magic_struct d = PG_MODULE_MAGIC_DATA;
    return &d;
}

/* ----------------------------------------------------------------
 * Forward declarations of Rust implementation functions
 * ---------------------------------------------------------------- */
extern void *kham_start_impl(const char *text, int len);
extern int   kham_gettoken_impl(void *state, const char **token, int *tokenlen);
extern void  kham_end_impl(void *state);

/* ----------------------------------------------------------------
 * startfunc — allocate parser state
 * Called from Rust kham_start() trampoline.
 * ---------------------------------------------------------------- */
Datum
kham_start_shim(PG_FUNCTION_ARGS)
{
    const char *input = (const char *) PG_GETARG_POINTER(0);
    int         len   = PG_GETARG_INT32(1);
    void       *state = kham_start_impl(input, len);
    if (state == NULL)
        ereport(ERROR,
                (errcode(ERRCODE_INTERNAL_ERROR),
                 errmsg("kham: failed to initialise parser state")));
    PG_RETURN_POINTER(state);
}

/* ----------------------------------------------------------------
 * gettoken — return next token; type 0 = end-of-document
 * ---------------------------------------------------------------- */
Datum
kham_gettoken_shim(PG_FUNCTION_ARGS)
{
    void   *state    = PG_GETARG_POINTER(0);
    char  **token    = (char **) PG_GETARG_POINTER(1);
    int    *tokenlen = (int *)   PG_GETARG_POINTER(2);
    int     type     = kham_gettoken_impl(state, (const char **) token, tokenlen);
    PG_RETURN_INT32(type);
}

/* ----------------------------------------------------------------
 * endfunc — release parser state
 * ---------------------------------------------------------------- */
Datum
kham_end_shim(PG_FUNCTION_ARGS)
{
    void *state = PG_GETARG_POINTER(0);
    kham_end_impl(state);
    PG_RETURN_VOID();
}

/* ----------------------------------------------------------------
 * lextypes — token-type description table
 * ---------------------------------------------------------------- */
Datum
kham_lextypes_shim(PG_FUNCTION_ARGS)
{
    LexDescr *list = (LexDescr *) palloc(7 * sizeof(LexDescr));

    list[0].lexid = 1; list[0].alias = pstrdup("thai");    list[0].descr = pstrdup("Thai word");
    list[1].lexid = 2; list[1].alias = pstrdup("latin");   list[1].descr = pstrdup("Latin script token");
    list[2].lexid = 3; list[2].alias = pstrdup("number");  list[2].descr = pstrdup("Numeric token");
    list[3].lexid = 4; list[3].alias = pstrdup("punct");   list[3].descr = pstrdup("Punctuation");
    list[4].lexid = 5; list[4].alias = pstrdup("emoji");   list[4].descr = pstrdup("Emoji token");
    list[5].lexid = 6; list[5].alias = pstrdup("unknown"); list[5].descr = pstrdup("Unknown / OOV token");
    list[6].lexid = 0; list[6].alias = NULL;                list[6].descr = NULL;

    PG_RETURN_POINTER(list);
}
