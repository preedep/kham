#ifndef KHAM_H
#define KHAM_H

#pragma once

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Heap-allocated array of null-terminated token strings.
 *
 * Must be freed with [`kham_tokens_free`].
 */
typedef struct KhamTokens {
  /**
   * Pointer to an array of `len` null-terminated UTF-8 strings.
   */
  char **words;
  /**
   * Number of tokens.
   */
  uintptr_t len;
} KhamTokens;

/**
 * A single token with text, byte/char span, and kind.
 *
 * All pointer fields are heap-allocated and owned by the containing
 * [`KhamTokenList`]. Free the list with [`kham_token_list_free`] — do not
 * free individual fields directly.
 */
typedef struct KhamToken {
  /**
   * Null-terminated UTF-8 token text.
   */
  char *text;
  /**
   * Start byte offset in the original UTF-8 input string.
   */
  uintptr_t byte_start;
  /**
   * End byte offset in the original UTF-8 input string.
   */
  uintptr_t byte_end;
  /**
   * Start Unicode scalar-value (char) offset in the original input string.
   */
  uintptr_t char_start;
  /**
   * End Unicode scalar-value (char) offset in the original input string.
   */
  uintptr_t char_end;
  /**
   * Null-terminated token kind string: `"Thai"`, `"Latin"`, `"Number"`,
   * `"Punctuation"`, `"Emoji"`, `"Whitespace"`, or `"Unknown"`.
   *
   * Note: `"Person"`, `"Place"`, and `"Org"` are never produced by
   * [`kham_segment_tokens`] because NE tagging is not part of the basic
   * segmentation pipeline. Use [`kham_fts_segment`] to obtain Named tokens.
   */
  char *kind;
} KhamToken;

/**
 * Heap-allocated array of [`KhamToken`] values.
 *
 * Must be freed with [`kham_token_list_free`].
 */
typedef struct KhamTokenList {
  /**
   * Pointer to an array of `len` [`KhamToken`] values.
   */
  struct KhamToken *tokens;
  /**
   * Number of tokens.
   */
  uintptr_t len;
} KhamTokenList;

/**
 * A single FTS token with stopword flag, synonym list, and trigrams.
 *
 * All pointer fields are heap-allocated and owned by the containing
 * [`KhamFtsTokenList`]. Free the list with [`kham_fts_token_list_free`] —
 * do not free individual fields directly.
 */
typedef struct KhamFtsToken {
  /**
   * Null-terminated UTF-8 token text.
   */
  char *text;
  /**
   * Ordinal position in the non-whitespace token sequence (0-based).
   */
  uintptr_t position;
  /**
   * Null-terminated token kind string.
   *
   * Possible values: `"Thai"`, `"Latin"`, `"Number"`, `"Punctuation"`,
   * `"Emoji"`, `"Whitespace"`, `"Unknown"`.
   * Named entity tokens produced by the NE gazetteer use `"Person"`,
   * `"Place"`, or `"Org"` instead of `"Thai"`.
   */
  char *kind;
  /**
   * `true` if this token matches the built-in stopword list.
   */
  bool is_stop;
  /**
   * Heap-allocated array of `synonyms_len` null-terminated synonym strings.
   */
  char **synonyms;
  /**
   * Number of entries in `synonyms`.
   */
  uintptr_t synonyms_len;
  /**
   * Heap-allocated array of `trigrams_len` null-terminated trigram strings.
   * Populated only for `TokenKind::Unknown` tokens.
   */
  char **trigrams;
  /**
   * Number of entries in `trigrams`.
   */
  uintptr_t trigrams_len;
} KhamFtsToken;

/**
 * Heap-allocated array of [`KhamFtsToken`] values.
 *
 * Must be freed with [`kham_fts_token_list_free`].
 */
typedef struct KhamFtsTokenList {
  /**
   * Pointer to an array of `len` [`KhamFtsToken`] values.
   */
  struct KhamFtsToken *tokens;
  /**
   * Number of tokens.
   */
  uintptr_t len;
} KhamFtsTokenList;

/**
 * Segment `text` into Thai tokens, returning an array of token strings.
 *
 * # Safety
 *
 * * `text` must be a valid null-terminated UTF-8 string.
 * * The returned pointer must be freed with [`kham_tokens_free`].
 * * Returns `NULL` if `text` is null or contains invalid UTF-8.
 */
struct KhamTokens *kham_segment(const char *text);

/**
 * Free a [`KhamTokens`] value returned by [`kham_segment`].
 *
 * # Safety
 *
 * * `tokens` must have been allocated by [`kham_segment`].
 * * Must not be called more than once on the same pointer.
 * * Passing `NULL` is safe (no-op).
 */
void kham_tokens_free(struct KhamTokens *tokens);

/**
 * Segment `text` into tokens, returning full span and kind information.
 *
 * # Safety
 *
 * * `text` must be a valid null-terminated UTF-8 string.
 * * The returned pointer must be freed with [`kham_token_list_free`].
 * * Returns `NULL` if `text` is null or contains invalid UTF-8.
 */
struct KhamTokenList *kham_segment_tokens(const char *text);

/**
 * Free a [`KhamTokenList`] value returned by [`kham_segment_tokens`].
 *
 * # Safety
 *
 * * `list` must have been allocated by [`kham_segment_tokens`].
 * * Must not be called more than once on the same pointer.
 * * Passing `NULL` is safe (no-op).
 */
void kham_token_list_free(struct KhamTokenList *list);

/**
 * Segment `text` through the FTS pipeline and return annotated tokens.
 *
 * Uses the built-in stopword list, no synonyms, and trigram size 3.
 *
 * # Safety
 *
 * * `text` must be a valid null-terminated UTF-8 string.
 * * The returned pointer must be freed with [`kham_fts_token_list_free`].
 * * Returns `NULL` if `text` is null or contains invalid UTF-8.
 */
struct KhamFtsTokenList *kham_fts_segment(const char *text);

/**
 * Free a [`KhamFtsTokenList`] value returned by [`kham_fts_segment`].
 *
 * # Safety
 *
 * * `list` must have been allocated by [`kham_fts_segment`].
 * * Must not be called more than once on the same pointer.
 * * Passing `NULL` is safe (no-op).
 */
void kham_fts_token_list_free(struct KhamFtsTokenList *list);

/**
 * Collect all FTS lexemes for `text` as a flat null-terminated string array.
 *
 * Lexemes are: non-stop token texts, plus synonym expansions and trigrams for
 * unknown tokens. Writes the count to `*out_len`.
 *
 * # Safety
 *
 * * `text` must be a valid null-terminated UTF-8 string.
 * * `out_len` must be a valid non-null pointer to a `usize`.
 * * The returned pointer must be freed with [`kham_fts_lexemes_free`] using
 *   the same `len` written to `*out_len`.
 * * Returns `NULL` if `text` is null, `out_len` is null, or input is invalid UTF-8.
 */
char **kham_fts_lexemes(const char *text, uintptr_t *out_len);

/**
 * Free a lexeme array returned by [`kham_fts_lexemes`].
 *
 * # Safety
 *
 * * `lexemes` must have been allocated by [`kham_fts_lexemes`] with the
 *   matching `len` written to `*out_len`.
 * * Must not be called more than once on the same pointer.
 * * Passing `NULL` is safe (no-op).
 */
void kham_fts_lexemes_free(char **lexemes, uintptr_t len);

#endif  /* KHAM_H */
