/*
 * shim.c — PostgreSQL fmgr bridge for the kham text-search parser.
 *
 * Handles all PG macro expansion (PG_MODULE_MAGIC, PG_FUNCTION_INFO_V1,
 * PG_GETARG_*, PG_RETURN_*) and palloc-based memory management so that the
 * Rust implementation in lib.rs only deals with simple C types.
 */

#include "postgres.h"
#include "varatt.h"
#include "fmgr.h"
#include "tsearch/ts_public.h"
#include "utils/palloc.h"

PG_MODULE_MAGIC;

/* ----------------------------------------------------------------
 * Forward declarations of Rust implementation functions
 * ---------------------------------------------------------------- */

extern void *kham_start_impl(const char *text, int len);
extern int   kham_gettoken_impl(void *state, const char **token, int *tokenlen);
extern void  kham_end_impl(void *state);

/* ----------------------------------------------------------------
 * startfunc — allocate parser state for one document
 * ---------------------------------------------------------------- */
PG_FUNCTION_INFO_V1(kham_start);
Datum
kham_start(PG_FUNCTION_ARGS)
{
    text *input = PG_GETARG_TEXT_PP(0);
    void *state = kham_start_impl(VARDATA_ANY(input), (int) VARSIZE_ANY_EXHDR(input));
    if (state == NULL)
        ereport(ERROR,
                (errcode(ERRCODE_INTERNAL_ERROR),
                 errmsg("kham: failed to initialise parser state")));
    PG_RETURN_POINTER(state);
}

/* ----------------------------------------------------------------
 * gettoken — return next token; type 0 signals end-of-document
 * ---------------------------------------------------------------- */
PG_FUNCTION_INFO_V1(kham_gettoken);
Datum
kham_gettoken(PG_FUNCTION_ARGS)
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
PG_FUNCTION_INFO_V1(kham_end);
Datum
kham_end(PG_FUNCTION_ARGS)
{
    void *state = PG_GETARG_POINTER(0);
    kham_end_impl(state);
    PG_RETURN_VOID();
}

/* ----------------------------------------------------------------
 * lextypes — token-type description table
 *
 * Returns a palloc'd LexDescr array terminated by { lexid=0, NULL, NULL }.
 * Must match the type integers returned by kham_gettoken_impl.
 * ---------------------------------------------------------------- */
PG_FUNCTION_INFO_V1(kham_lextypes);
Datum
kham_lextypes(PG_FUNCTION_ARGS)
{
    LexDescr *list = (LexDescr *) palloc(7 * sizeof(LexDescr));

    list[0].lexid = 1;
    list[0].alias = pstrdup("thai");
    list[0].descr = pstrdup("Thai word");

    list[1].lexid = 2;
    list[1].alias = pstrdup("latin");
    list[1].descr = pstrdup("Latin script token");

    list[2].lexid = 3;
    list[2].alias = pstrdup("number");
    list[2].descr = pstrdup("Numeric token");

    list[3].lexid = 4;
    list[3].alias = pstrdup("punct");
    list[3].descr = pstrdup("Punctuation");

    list[4].lexid = 5;
    list[4].alias = pstrdup("emoji");
    list[4].descr = pstrdup("Emoji token");

    list[5].lexid = 6;
    list[5].alias = pstrdup("unknown");
    list[5].descr = pstrdup("Unknown / OOV token");

    /* sentinel — terminates the array */
    list[6].lexid = 0;
    list[6].alias = NULL;
    list[6].descr = NULL;

    PG_RETURN_POINTER(list);
}
