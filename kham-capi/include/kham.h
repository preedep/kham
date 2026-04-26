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
 * A single FTS token with stopword flag, synonym list, trigrams, POS, NE, and romanization.
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
   * RTGS romanization of the token text. Never NULL; equals the original
   * text for non-Thai or out-of-vocabulary tokens.
   */
  char *roman;
  /**
   * POS tag string, or `NULL` if the word is not in the POS dictionary.
   *
   * Possible values: `"Noun"`, `"Verb"`, `"Adj"`, `"Adv"`, `"Particle"`,
   * `"ProperNoun"`, `"Pronoun"`, `"Numeral"`, `"Classifier"`,
   * `"Conjunction"`, `"Auxiliary"`, `"Determiner"`, `"Preposition"`.
   */
  char *pos;
  /**
   * Named entity category, or `NULL` if not in the NE gazetteer.
   *
   * Possible values: `"Person"`, `"Place"`, `"Org"`.
   */
  char *ne;
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
 * A token text paired with its RTGS romanization.
 *
 * Owned by the enclosing [`KhamRomanTokenList`]; do not free fields directly.
 */
typedef struct KhamRomanToken {
  /**
   * Null-terminated original token text.
   */
  char *text;
  /**
   * Null-terminated RTGS romanization.
   */
  char *roman;
} KhamRomanToken;

/**
 * Heap-allocated array of [`KhamRomanToken`] values.
 *
 * Must be freed with [`kham_roman_token_list_free`].
 */
typedef struct KhamRomanTokenList {
  /**
   * Pointer to an array of `len` [`KhamRomanToken`] values.
   */
  struct KhamRomanToken *tokens;
  /**
   * Number of tokens.
   */
  uintptr_t len;
} KhamRomanTokenList;

/**
 * A sentence span with text and Unicode char offsets.
 *
 * Owned by the enclosing [`KhamSentenceList`]; do not free fields directly.
 */
typedef struct KhamSentence {
  /**
   * Null-terminated UTF-8 sentence text.
   */
  char *text;
  /**
   * Start Unicode scalar-value offset in the original input string.
   */
  uintptr_t char_start;
  /**
   * End Unicode scalar-value offset in the original input string.
   */
  uintptr_t char_end;
} KhamSentence;

/**
 * Heap-allocated array of [`KhamSentence`] values.
 *
 * Must be freed with [`kham_sentence_list_free`].
 */
typedef struct KhamSentenceList {
  /**
   * Pointer to an array of `len` [`KhamSentence`] values.
   */
  struct KhamSentence *sentences;
  /**
   * Number of sentences.
   */
  uintptr_t len;
} KhamSentenceList;

/**
 * A parsed Thai Baht currency amount.
 */
typedef struct KhamBahtAmount {
  /**
   * Whole baht amount.
   */
  uint64_t baht;
  /**
   * Satang (0–99).
   */
  uint8_t satang;
} KhamBahtAmount;

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
 * Each token includes POS tag, NE category, and RTGS romanization.
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

/**
 * Free a null-terminated string returned by any kham function that returns
 * `char *` (e.g. [`kham_normalize`], [`kham_soundex`], [`kham_number_to_thai_word`]).
 *
 * # Safety
 *
 * * `s` must have been allocated by a kham function.
 * * Must not be called more than once on the same pointer.
 * * Passing `NULL` is safe (no-op).
 */
void kham_string_free(char *s);

/**
 * Normalize Thai text: deduplicate tone marks and compose sara am.
 *
 * Returns a newly allocated null-terminated UTF-8 string.
 * Free with [`kham_string_free`].
 *
 * # Safety
 *
 * * `text` must be a valid null-terminated UTF-8 string.
 * * Returns `NULL` if `text` is null or contains invalid UTF-8.
 */
char *kham_normalize(const char *text);

/**
 * Segment `text` and return each token paired with its RTGS romanization.
 *
 * # Safety
 *
 * * `text` must be a valid null-terminated UTF-8 string.
 * * The returned pointer must be freed with [`kham_roman_token_list_free`].
 * * Returns `NULL` if `text` is null or contains invalid UTF-8.
 */
struct KhamRomanTokenList *kham_romanize(const char *text);

/**
 * Free a [`KhamRomanTokenList`] value returned by [`kham_romanize`].
 *
 * # Safety
 *
 * * `list` must have been allocated by [`kham_romanize`].
 * * Must not be called more than once on the same pointer.
 * * Passing `NULL` is safe (no-op).
 */
void kham_roman_token_list_free(struct KhamRomanTokenList *list);

/**
 * Split `text` into sentence spans.
 *
 * Boundaries: Thai markers (ฯ ๚ ๛), newlines, and Western punctuation
 * (`! ? .` followed by a space).
 *
 * # Safety
 *
 * * `text` must be a valid null-terminated UTF-8 string.
 * * The returned pointer must be freed with [`kham_sentence_list_free`].
 * * Returns `NULL` if `text` is null or contains invalid UTF-8.
 */
struct KhamSentenceList *kham_split_sentences(const char *text);

/**
 * Free a [`KhamSentenceList`] value returned by [`kham_split_sentences`].
 *
 * # Safety
 *
 * * `list` must have been allocated by [`kham_split_sentences`].
 * * Must not be called more than once on the same pointer.
 * * Passing `NULL` is safe (no-op).
 */
void kham_sentence_list_free(struct KhamSentenceList *list);

/**
 * Encode `word` using the specified phonetic algorithm.
 *
 * `algo` is one of `"lk82"` (default), `"udom83"`, or `"metasound"`.
 * Pass `NULL` for `algo` to use the default (`"lk82"`).
 *
 * Returns a newly allocated null-terminated string.
 * Free with [`kham_string_free`].
 *
 * # Safety
 *
 * * `word` must be a valid null-terminated UTF-8 string.
 * * `algo` may be `NULL` (→ lk82) or a valid null-terminated string.
 * * Returns `NULL` if `word` is null or contains invalid UTF-8.
 */
char *kham_soundex(const char *word, const char *algo);

/**
 * Return `true` if `a` and `b` sound alike under the given algorithm.
 *
 * `algo` is one of `"lk82"` (default), `"udom83"`, or `"metasound"`.
 * Pass `NULL` for `algo` to use the default.
 *
 * # Safety
 *
 * * `a` and `b` must be valid null-terminated UTF-8 strings.
 * * `algo` may be `NULL`.
 * * Returns `false` if either input is null or contains invalid UTF-8.
 */
bool kham_sounds_like(const char *a, const char *b, const char *algo);

/**
 * Compute the Thai–English cross-language soundex for `word`.
 *
 * Accepts both Thai script and ASCII input.
 * Returns a newly allocated null-terminated string.
 * Free with [`kham_string_free`].
 *
 * # Safety
 *
 * * `word` must be a valid null-terminated UTF-8 string.
 * * Returns `NULL` if `word` is null or contains invalid UTF-8.
 */
char *kham_thai_english_soundex(const char *word);

/**
 * Return `true` if `a` and `b` are phonetically similar across Thai and English.
 *
 * # Safety
 *
 * * `a` and `b` must be valid null-terminated UTF-8 strings.
 * * Returns `false` if either input is null or contains invalid UTF-8.
 */
bool kham_sounds_like_cross_lang(const char *a, const char *b);

/**
 * Convert Thai digit characters (๐–๙) in `text` to ASCII digits.
 *
 * Returns a newly allocated null-terminated UTF-8 string.
 * Free with [`kham_string_free`].
 *
 * # Safety
 *
 * * `text` must be a valid null-terminated UTF-8 string.
 * * Returns `NULL` if `text` is null or contains invalid UTF-8.
 */
char *kham_thai_digits_to_ascii(const char *text);

/**
 * Convert the integer `n` to its Thai cardinal word representation.
 *
 * Returns a newly allocated null-terminated UTF-8 string.
 * Free with [`kham_string_free`].
 */
char *kham_number_to_thai_word(uint64_t n);

/**
 * Parse a Thai cardinal number word into an integer.
 *
 * On success writes the value to `*out_n` and returns `true`.
 * Returns `false` (and does not write to `*out_n`) if the text is not a
 * valid Thai number word.
 *
 * # Safety
 *
 * * `text` must be a valid null-terminated UTF-8 string.
 * * `out_n` must be a valid non-null pointer to a `uint64_t`.
 * * Returns `false` if `text` or `out_n` is null.
 */
bool kham_thai_word_to_number(const char *text, uint64_t *out_n);

/**
 * Render a Baht amount as Thai currency text.
 *
 * `satang` must be 0–99.
 * Returns a newly allocated null-terminated UTF-8 string.
 * Free with [`kham_string_free`].
 */
char *kham_number_to_baht_text(uint64_t baht, uint8_t satang);

/**
 * Parse a Thai Baht currency string.
 *
 * Returns a heap-allocated [`KhamBahtAmount`] on success, or `NULL` if the
 * string is not a valid Thai Baht amount.
 * Free with [`kham_baht_amount_free`].
 *
 * # Safety
 *
 * * `text` must be a valid null-terminated UTF-8 string.
 * * Returns `NULL` if `text` is null, contains invalid UTF-8, or is not a
 *   valid Thai Baht string.
 */
struct KhamBahtAmount *kham_parse_baht_text(const char *text);

/**
 * Free a [`KhamBahtAmount`] returned by [`kham_parse_baht_text`].
 *
 * # Safety
 *
 * * `amt` must have been allocated by [`kham_parse_baht_text`].
 * * Must not be called more than once on the same pointer.
 * * Passing `NULL` is safe (no-op).
 */
void kham_baht_amount_free(struct KhamBahtAmount *amt);

#endif  /* KHAM_H */
