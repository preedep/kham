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
#include "nodes/pg_list.h"
#include "nodes/parsenodes.h"
#include "nodes/value.h"
#include "tsearch/ts_public.h"
#include "tsearch/ts_type.h"
#include "utils/palloc.h"

/* ----------------------------------------------------------------
 * Module magic data — must be compiled against the target PG headers
 * so all version fields are correct for the running server.
 * Called from the Rust Pg_magic_func() trampoline in lib.rs.
 * ---------------------------------------------------------------- */
const Pg_magic_struct *
kham_pg_magic_impl(void)
{
    /* PG_MODULE_MAGIC_DATA is function-like (variadic) when PG_MODULE_ABI_DATA
     * is defined (PG 17 Homebrew / PG 18+).  PGDG PG 17 uses the object-like
     * form without parentheses. */
#ifdef PG_MODULE_ABI_DATA
    static const Pg_magic_struct d = PG_MODULE_MAGIC_DATA();
#else
    static const Pg_magic_struct d = PG_MODULE_MAGIC_DATA;
#endif
    return &d;
}

/* ----------------------------------------------------------------
 * Forward declarations of Rust implementation functions
 * ---------------------------------------------------------------- */
extern void *kham_start_impl(const char *text, int len);
extern int   kham_gettoken_impl(void *state, const char **token, int *tokenlen);
extern void  kham_end_impl(void *state);

/*
 * KhamDictOut — output buffer filled by kham_dict_lexize_impl (Rust).
 * count: number of valid lexeme slots (0 = stopword / error).
 * words: up to 6 null-terminated UTF-8 strings, each at most 127 bytes.
 */
typedef struct
{
    int  count;
    char words[6][128];
} KhamDictOut;

extern int kham_dict_lexize_impl(const char *token, int token_len, KhamDictOut *out);

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
 * headline — mark words that match the tsquery for ts_headline()
 *
 * Args: HeadlineParsedText*(internal), opts List*(internal), TSQuery
 *
 * In PG17 the prsheadline callback must:
 *   1. Fill prs->startsel / stopsel / fragdelim (palloc'd).
 *   2. Mark prs->words[i].in = 1 for tokens to include in output.
 *   3. Mark prs->words[i].selected = 1 for tokens to highlight.
 *
 * We include all non-skip tokens (in=1, whole-document mode) and
 * highlight tokens that exactly or prefix-match a QI_VAL operand.
 * StartSel / StopSel / FragmentDelimiter can be overridden via opts.
 * ---------------------------------------------------------------- */
Datum
kham_headline_shim(PG_FUNCTION_ARGS)
{
    HeadlineParsedText *prs     = (HeadlineParsedText *) PG_GETARG_POINTER(0);
    List               *prsopts = (List *) PG_GETARG_POINTER(1);
    TSQuery             query   = PG_GETARG_TSQUERY(2);
    ListCell           *l;
    int                 i, qi;

    /* default highlight markers */
    const char *startsel   = "<b>";
    const char *stopsel    = "</b>";
    const char *fragdelim  = " ... ";
    int16       startsellen;
    int16       stopsellen;
    int16       fragdelimlen;

    /* parse caller-supplied options (StartSel, StopSel, FragmentDelimiter) */
    foreach(l, prsopts)
    {
        DefElem *defel = (DefElem *) lfirst(l);
        if (pg_strcasecmp(defel->defname, "startsel") == 0)
            startsel = strVal(defel->arg);
        else if (pg_strcasecmp(defel->defname, "stopsel") == 0)
            stopsel = strVal(defel->arg);
        else if (pg_strcasecmp(defel->defname, "fragmentdelimiter") == 0)
            fragdelim = strVal(defel->arg);
        /* other options (MaxWords, MinWords, MaxFragments…) are accepted
         * silently — we always output the full document */
    }

    startsellen  = (int16) strlen(startsel);
    stopsellen   = (int16) strlen(stopsel);
    fragdelimlen = (int16) strlen(fragdelim);

    /* copy selector strings into palloc'd memory (required by PG) */
    prs->startsel = palloc(startsellen + 1);
    memcpy(prs->startsel, startsel, startsellen + 1);
    prs->startsellen = startsellen;

    prs->stopsel = palloc(stopsellen + 1);
    memcpy(prs->stopsel, stopsel, stopsellen + 1);
    prs->stopsellen = stopsellen;

    prs->fragdelim = palloc(fragdelimlen + 1);
    memcpy(prs->fragdelim, fragdelim, fragdelimlen + 1);
    prs->fragdelimlen = fragdelimlen;

    if (prs->words == NULL || prs->curwords == 0)
        PG_RETURN_POINTER(prs);

    if (query->size > 0)
    {
        QueryItem  *items    = GETQUERY(query);
        const char *operands = GETOPERAND(query);

        for (i = 0; i < prs->curwords; i++)
        {
            HeadlineWordEntry *entry = &prs->words[i];
            if (entry->skip || entry->repeated)
                continue;

            entry->in = 1;  /* include every token in the output */

            for (qi = 0; qi < query->size; qi++)
            {
                QueryOperand *op;
                const char   *val;

                if (items[qi].type != QI_VAL)
                    continue;

                op  = &items[qi].qoperand;
                val = operands + op->distance;

                if (op->prefix)
                {
                    if ((int) op->length <= (int) entry->len &&
                        strncasecmp(entry->word, val, op->length) == 0)
                    {
                        entry->selected = 1;
                        break;
                    }
                }
                else
                {
                    if ((int) op->length == (int) entry->len &&
                        strncasecmp(entry->word, val, op->length) == 0)
                    {
                        entry->selected = 1;
                        break;
                    }
                }
            }
        }
    }
    else
    {
        /* no query operands — mark all words as in but not selected */
        for (i = 0; i < prs->curwords; i++)
        {
            if (!prs->words[i].skip && !prs->words[i].repeated)
                prs->words[i].in = 1;
        }
    }

    PG_RETURN_POINTER(prs);
}

/* ----------------------------------------------------------------
 * kham_dict_lexize — custom dictionary: word + soundex + RTGS lexemes
 *
 * Signature (internal, internal, internal, internal) → internal
 * Args:
 *   arg0 = dict state (NULL — no INIT function)
 *   arg1 = token text (char *)
 *   arg2 = token length (int32)
 *   arg3 = List* of subsequent tokens for multi-word recognition (PG16+)
 *          NOTE: In PG 13 and earlier this was a bool "isNull" flag.
 *          Since PG 16 it is a List* pointer (always non-NULL for normal
 *          tokens).  We do not use multi-word recognition, so arg3 is
 *          ignored entirely.
 *
 * Returns a palloc'd TSLexeme[] containing the normalised word plus
 * any soundex / RTGS synonyms computed by Rust.  The array is
 * NULL-terminated (last entry has lexeme=NULL).
 * Returns NULL to mark the token as a stopword (not indexed).
 * ---------------------------------------------------------------- */
Datum
kham_dict_lexize_shim(PG_FUNCTION_ARGS)
{
    /* arg0 = dict state (NULL — we use no INIT function)
     * arg1 = token text (char *)
     * arg2 = token length (int32)
     * arg3 = List* of subsequent tokens for multi-word recognition (PG16+, ignored) */
    const char  *token  = (const char *) PG_GETARG_POINTER(1);
    int          len    = PG_GETARG_INT32(2);
    KhamDictOut  out;
    TSLexeme    *res;
    int          i;

    /* In PG 16+, arg3 is a List* of subsequent tokens for multi-word
     * recognition (not a bool isNull flag as in older PG).  We do not
     * use multi-word recognition, so we ignore arg3 entirely.
     * End-of-input finalization: token pointer is NULL → return NULL. */
    if (token == NULL || len <= 0)
        PG_RETURN_POINTER(NULL);

    memset(&out, 0, sizeof(out));
    kham_dict_lexize_impl(token, len, &out);

    if (out.count <= 0)
        PG_RETURN_POINTER(NULL);   /* treat as stopword */

    res = (TSLexeme *) palloc0(sizeof(TSLexeme) * (out.count + 1));
    for (i = 0; i < out.count; i++)
        res[i].lexeme = pstrdup(out.words[i]);
    res[out.count].lexeme = NULL;  /* NULL-terminator required by PG */

    PG_RETURN_POINTER(res);
}

/* ----------------------------------------------------------------
 * lextypes — token-type description table
 * ---------------------------------------------------------------- */
Datum
kham_lextypes_shim(PG_FUNCTION_ARGS)
{
    LexDescr *list = (LexDescr *) palloc(8 * sizeof(LexDescr));

    list[0].lexid = 1; list[0].alias = pstrdup("thai");    list[0].descr = pstrdup("Thai word");
    list[1].lexid = 2; list[1].alias = pstrdup("latin");   list[1].descr = pstrdup("Latin script token");
    list[2].lexid = 3; list[2].alias = pstrdup("number");  list[2].descr = pstrdup("Numeric token");
    list[3].lexid = 4; list[3].alias = pstrdup("punct");   list[3].descr = pstrdup("Punctuation");
    list[4].lexid = 5; list[4].alias = pstrdup("emoji");   list[4].descr = pstrdup("Emoji token");
    list[5].lexid = 6; list[5].alias = pstrdup("unknown"); list[5].descr = pstrdup("Unknown / OOV token");
    list[6].lexid = 7; list[6].alias = pstrdup("named");   list[6].descr = pstrdup("Named entity token");
    list[7].lexid = 0; list[7].alias = NULL;                list[7].descr = NULL;

    PG_RETURN_POINTER(list);
}
