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
        assert kham.segment("กินข้าวกับปลา") == ["กิน", "ข้าว", "กับ", "ปลา"]

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
        known = {"Thai", "Latin", "Number", "Punctuation", "Emoji", "Whitespace", "Unknown"}
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
            ("กินข้าวกับปลา",   ["กิน", "ข้าว", "กับ", "ปลา"]),
            ("สวัสดีชาวโลก",    ["สวัสดี", "ชาว", "โลก"]),
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
