"""
pytest suite for the kham Python bindings.

Focus: segment_tokens() — asserts that char_start/char_end round-trip
correctly against Python's native string indexing, and that all Token
attributes are present and consistent.

Run:
    source .venv/bin/activate
    pytest kham-python/tests/test_kham.py -v
"""

import kham
import pytest


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def tokens(text: str):
    """Return segment_tokens() output for *text*."""
    return kham.segment_tokens(text)


def char_slice(text: str, t) -> str:
    """Re-extract token text using Python string indexing and char_span."""
    return text[t.char_start:t.char_end]


# ---------------------------------------------------------------------------
# segment() — basic sanity
# ---------------------------------------------------------------------------

class TestSegment:
    def test_pure_thai(self):
        assert kham.segment("กินข้าวกับปลา") == ["กินข้าว", "กับ", "ปลา"]

    def test_mixed_script(self):
        result = kham.segment("ธนาคาร100แห่ง")
        assert result == ["ธนาคาร", "100", "แห่ง"]

    def test_empty_string(self):
        assert kham.segment("") == []

    def test_returns_list_of_str(self):
        for tok in kham.segment("กินข้าว"):
            assert isinstance(tok, str)


# ---------------------------------------------------------------------------
# segment_tokens() — Token attributes present
# ---------------------------------------------------------------------------

class TestTokenAttributes:
    def test_has_all_fields(self):
        toks = tokens("กินข้าว")
        assert toks, "expected at least one token"
        t = toks[0]
        assert hasattr(t, "text")
        assert hasattr(t, "byte_start")
        assert hasattr(t, "byte_end")
        assert hasattr(t, "char_start")
        assert hasattr(t, "char_end")
        assert hasattr(t, "kind")

    def test_text_is_str(self):
        for t in tokens("กินข้าว"):
            assert isinstance(t.text, str)

    def test_kind_is_known_value(self):
        # segment_tokens() never produces Named kinds ("Person"/"Place"/"Org")
        # because NE tagging is not part of the basic pipeline — those are FTS-only.
        # They are included here so the assertion stays valid if a future API
        # exposes NE-tagged tokens through this binding.
        known = {
            "Thai", "Latin", "Number", "Punctuation",
            "Emoji", "Whitespace", "Unknown",
            "Person", "Place", "Org",
        }
        for t in tokens("กินข้าว100hello"):
            assert t.kind in known, f"unexpected kind {t.kind!r}"

    def test_repr_does_not_raise(self):
        for t in tokens("กินข้าว"):
            assert repr(t)


# ---------------------------------------------------------------------------
# char_span round-trip — the primary correctness test
# ---------------------------------------------------------------------------

ROUNDTRIP_CASES = [
    # (description, input)
    ("pure_thai",          "กินข้าวกับปลา"),
    ("mixed_thai_number",  "ธนาคาร100แห่ง"),
    ("mixed_thai_latin",   "สวัสดีhelloโลก"),
    ("all_mixed",          "ธนาคาร100แห่งhello42"),
    ("single_thai_char",   "ก"),
    ("single_latin_char",  "A"),
    ("single_digit",       "9"),
]


@pytest.mark.parametrize("desc,text", ROUNDTRIP_CASES, ids=[c[0] for c in ROUNDTRIP_CASES])
def test_char_span_roundtrip(desc, text):
    """char_start/char_end must reproduce token.text via Python string indexing."""
    toks = tokens(text)
    assert toks, f"no tokens for {text!r}"
    for t in toks:
        extracted = char_slice(text, t)
        assert extracted == t.text, (
            f"char_span [{t.char_start}:{t.char_end}] → {extracted!r} "
            f"but token.text={t.text!r}  (input={text!r})"
        )


# ---------------------------------------------------------------------------
# char_span ordering and contiguity
# ---------------------------------------------------------------------------

class TestCharSpanInvariants:
    def test_start_less_than_end(self):
        for t in tokens("กินข้าวกับปลา"):
            assert t.char_start < t.char_end, f"empty or inverted span: {t!r}"

    def test_spans_are_contiguous(self):
        """Consecutive tokens must share a boundary (no gaps, no overlaps)."""
        toks = tokens("กินข้าวกับปลา")
        for a, b in zip(toks, toks[1:]):
            assert a.char_end == b.char_start, (
                f"gap/overlap between {a!r} and {b!r}"
            )

    def test_first_span_starts_at_zero(self):
        toks = tokens("กินข้าว")
        assert toks[0].char_start == 0

    def test_last_span_ends_at_char_count(self):
        text = "กินข้าวกับปลา"
        toks = tokens(text)
        assert toks[-1].char_end == len(text)  # Python len() counts chars, not bytes

    def test_mixed_script_contiguous(self):
        text = "ธนาคาร100แห่ง"
        toks = tokens(text)
        for a, b in zip(toks, toks[1:]):
            assert a.char_end == b.char_start


# ---------------------------------------------------------------------------
# byte_span sanity
# ---------------------------------------------------------------------------

class TestByteSpan:
    def test_byte_span_slices_correctly(self):
        text = "ธนาคาร100แห่ง"
        raw = text.encode("utf-8")
        for t in tokens(text):
            chunk = raw[t.byte_start:t.byte_end].decode("utf-8")
            assert chunk == t.text, (
                f"byte_span [{t.byte_start}:{t.byte_end}] → {chunk!r} "
                f"but token.text={t.text!r}"
            )

    def test_byte_start_less_than_byte_end(self):
        for t in tokens("กินข้าวกับปลา"):
            assert t.byte_start < t.byte_end


# ---------------------------------------------------------------------------
# Known segmentation expectations
# ---------------------------------------------------------------------------

class TestKnownSplits:
    def test_pure_thai_texts(self):
        cases = [
            ("กินข้าวกับปลา",   ["กินข้าว", "กับ", "ปลา"]),
            ("สวัสดีชาวโลก",    ["สวัสดี", "ชาวโลก"]),
        ]
        for text, expected in cases:
            got = [t.text for t in tokens(text)]
            assert got == expected, f"wrong split for {text!r}: {got}"

    def test_mixed_script_kinds(self):
        toks = tokens("ธนาคาร100แห่ง")
        by_text = {t.text: t.kind for t in toks}
        assert by_text["ธนาคาร"] == "Thai"
        assert by_text["100"] == "Number"
        assert by_text["แห่ง"] == "Thai"

    def test_latin_kind(self):
        toks = tokens("สวัสดีhello")
        lat = next((t for t in toks if t.text == "hello"), None)
        assert lat is not None, "no 'hello' token"
        assert lat.kind == "Latin"

    def test_empty_input(self):
        assert tokens("") == []


# ---------------------------------------------------------------------------
# Edge cases
# ---------------------------------------------------------------------------

class TestEdgeCases:
    def test_single_thai_char_roundtrip(self):
        text = "ก"
        toks = tokens(text)
        assert len(toks) == 1
        assert char_slice(text, toks[0]) == "ก"

    def test_char_and_byte_agree_for_ascii(self):
        """ASCII-only: char offsets == byte offsets."""
        text = "hello123"
        for t in tokens(text):
            assert t.char_start == t.byte_start
            assert t.char_end == t.byte_end

    def test_thai_byte_offsets_differ_from_char_offsets(self):
        """Thai chars are 3 bytes each — byte offsets must exceed char offsets."""
        text = "กินข้าว"
        for t in tokens(text):
            span_chars = t.char_end - t.char_start
            span_bytes = t.byte_end - t.byte_start
            assert span_bytes >= span_chars, (
                f"byte span smaller than char span for {t.text!r}"
            )

    def test_reconstruction_equals_input(self):
        """Joining all token texts must reconstruct the original string."""
        text = "ธนาคาร100แห่งhello"
        rebuilt = "".join(t.text for t in tokens(text))
        assert rebuilt == text


# ---------------------------------------------------------------------------
# spell_did_you_mean
# ---------------------------------------------------------------------------

class TestSpellDidYouMean:
    def test_correct_word_returns_none(self):
        assert kham.spell_did_you_mean("กิน") is None

    def test_empty_input_returns_none(self):
        assert kham.spell_did_you_mean("") is None

    def test_returns_str_or_none(self):
        result = kham.spell_did_you_mean("กานข้าว")
        assert result is None or isinstance(result, str)

    def test_suggestion_is_non_empty_when_present(self):
        result = kham.spell_did_you_mean("กานข้าว")
        if result is not None:
            assert len(result) > 0


# ---------------------------------------------------------------------------
# spell_correct_text
# ---------------------------------------------------------------------------

class TestSpellCorrectText:
    def test_returns_str(self):
        assert isinstance(kham.spell_correct_text("กินข้าว"), str)

    def test_known_text_is_non_empty(self):
        assert kham.spell_correct_text("กินข้าว") != ""

    def test_empty_input_returns_empty(self):
        assert kham.spell_correct_text("") == ""

    def test_known_text_unchanged(self):
        # All tokens are known, so the result should reproduce the input text.
        result = kham.spell_correct_text("กินข้าวกับปลา")
        assert result == "กินข้าวกับปลา"


# ---------------------------------------------------------------------------
# romanize_sentence
# ---------------------------------------------------------------------------

class TestRomanizeSentence:
    def test_returns_str(self):
        assert isinstance(kham.romanize_sentence("กิน"), str)

    def test_thai_produces_ascii(self):
        result = kham.romanize_sentence("กิน")
        assert result.isascii(), f"expected ASCII, got {result!r}"

    def test_latin_passthrough(self):
        assert kham.romanize_sentence("hello") == "hello"

    def test_non_empty_for_thai(self):
        assert kham.romanize_sentence("กินข้าว") != ""

    def test_empty_input_returns_empty(self):
        assert kham.romanize_sentence("") == ""

    def test_number_passthrough(self):
        result = kham.romanize_sentence("100")
        assert result == "100"


# ---------------------------------------------------------------------------
# extract_phrases
# ---------------------------------------------------------------------------

class TestExtractPhrases:
    def test_returns_list(self):
        result = kham.extract_phrases("การพัฒนาซอฟต์แวร์เป็นสิ่งสำคัญ", 5)
        assert isinstance(result, list)

    def test_max_n_zero_returns_empty(self):
        result = kham.extract_phrases("การพัฒนาซอฟต์แวร์เป็นสิ่งสำคัญ", 0)
        assert result == []

    def test_empty_text_returns_empty(self):
        assert kham.extract_phrases("", 5) == []

    def test_respects_max_n(self):
        text = "การพัฒนาซอฟต์แวร์เป็นสิ่งสำคัญในยุคดิจิทัล"
        result = kham.extract_phrases(text, 2)
        assert len(result) <= 2

    def test_phrase_has_required_attributes(self):
        text = "การพัฒนาซอฟต์แวร์เป็นสิ่งสำคัญในยุคดิจิทัล"
        for p in kham.extract_phrases(text, 5):
            assert isinstance(p.word, str)
            assert isinstance(p.score, float)
            assert isinstance(p.count, int)
            assert p.word != ""
            assert p.score >= 0.0
            assert p.count >= 1

    def test_score_descending(self):
        text = "การพัฒนาซอฟต์แวร์เป็นสิ่งสำคัญในยุคดิจิทัล"
        phrases = kham.extract_phrases(text, 10)
        scores = [p.score for p in phrases]
        assert scores == sorted(scores, reverse=True), "phrases must be sorted by score descending"
