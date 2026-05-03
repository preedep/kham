#!/usr/bin/env python3
"""
kham-tnc NER + POS Validator sidecar.

Uses pythainlp's ThaiNER (WangchanBERTa-based) for Named Entity Recognition
and pythainlp's perceptron tagger for Part-of-Speech tagging.
pythainlp manages model downloads via its own corpus manager — no HuggingFace
login required.

Endpoints:
  GET  /health                   — liveness check
  POST /validate                 — predict NE + POS for a word in context
  GET  /validate?word=X&context= — same, via GET (browser-friendly)
"""

import os
import sys

import uvicorn
from fastapi import FastAPI, Query
from pydantic import BaseModel

# ── Config ───────────────────────────────────────────────────────────────────
# NER engine: "thainer-v2" (default, WangchanBERTa-based, no pycrfsuite needed)
# "thainer" uses CRF (requires pycrfsuite) — not available on Python 3.14
NER_ENGINE: str = os.getenv("NER_ENGINE", "thainer-v2")

# POS engine and corpus (perceptron is fast, no large model download)
# corpus "orchid_ud" maps ORCHID tagset to Universal Dependencies tags,
# which closely match kham-core's 13-category POS scheme (ADR-003).
POS_ENGINE: str = os.getenv("POS_ENGINE", "perceptron")
POS_CORPUS: str = os.getenv("POS_CORPUS", "orchid_ud")

# ── Label maps ───────────────────────────────────────────────────────────────

_NE_MAP: dict[str, str] = {
    "PERSON": "PERSON",
    "PER": "PERSON",
    "LOCATION": "PLACE",
    "LOC": "PLACE",
    "GPE": "PLACE",
    "FAC": "PLACE",
    "ORGANIZATION": "ORG",
    "ORG": "ORG",
    "DATE": "DATE",
    "TIME": "TIME",
    "MONEY": "MONEY",
    "PERCENT": "MISC",
    "MISC": "MISC",
}

# Maps UD / ORCHID_UD tags → kham-core 13-category scheme (ADR-003).
# Tags not in this map (e.g. X, INTJ) are dropped (None → no POS suggestion).
_POS_MAP: dict[str, str | None] = {
    "NOUN":  "NOUN",
    "VERB":  "VERB",
    "ADJ":   "ADJ",
    "ADV":   "ADV",
    "PROPN": "PROPN",
    "PRON":  "PRON",
    "NUM":   "NUM",
    "DET":   "DET",
    "ADP":   "ADP",
    "PART":  "PART",
    "CCONJ": "CCONJ",
    "SCONJ": "CCONJ",   # kham-core has no SCONJ; CCONJ is closest
    "AUX":   "VERB",    # kham-core has no AUX
    "PUNCT": "PUNCT",
    "SYM":   "PUNCT",
    "INTJ":  "PART",
    "X":     None,      # unknown / foreign — no suggestion
}

app = FastAPI(title="kham-tnc NER + POS Validator", version="1.2.0")
_ner_tagger = None   # loaded on startup
_pos_ready = False   # True after warm-up


@app.on_event("startup")
async def _load_models() -> None:
    global _ner_tagger, _pos_ready
    print(f"Loading ThaiNER model via pythainlp (engine={NER_ENGINE}) …")
    from pythainlp.tag import NER as ThaiNER  # noqa: PLC0415

    _ner_tagger = ThaiNER(engine=NER_ENGINE)
    _ner_tagger.tag("ทดสอบ")   # triggers download if not cached
    print("NER model ready.")

    print(f"Warming up POS tagger (engine={POS_ENGINE}, corpus={POS_CORPUS}) …")
    from pythainlp.tag import pos_tag          # noqa: PLC0415
    from pythainlp.tokenize import word_tokenize  # noqa: PLC0415

    pos_tag(word_tokenize("ทดสอบ"), engine=POS_ENGINE, corpus=POS_CORPUS)
    _pos_ready = True
    print("POS tagger ready.")


# ── Schemas ───────────────────────────────────────────────────────────────────

class ValidateRequest(BaseModel):
    word: str
    context: str = ""


class ValidateResponse(BaseModel):
    word: str
    ne: str | None = None           # kham-core NE tag, or None
    pos: str | None = None          # kham-core POS tag, or None
    confidence: float = 0.0         # 1.0 when at least one tag found
    raw_label: str | None = None    # original BIO NE label (e.g. "B-LOCATION")
    pos_raw_label: str | None = None  # raw POS label before mapping (e.g. "PROPN")


# ── Prediction logic ──────────────────────────────────────────────────────────

def _predict(word: str, context: str) -> ValidateResponse:
    """
    Run NER and POS tagging on `context` (or `word` alone if no context) and
    return the best matching NE + POS for the target `word`.

    Each stage is wrapped independently — a POS failure never drops the NE
    result, and vice versa.  Overlap strategy: exact token match preferred;
    fallback to first substring overlap.
    """
    text = context.strip() or word

    ne: str | None = None
    raw_ne_label: str | None = None
    pos: str | None = None
    raw_pos_label: str | None = None

    # ── NER ──────────────────────────────────────────────────────────────────
    try:
        tagged_ner: list[tuple[str, str]] = _ner_tagger.tag(text)
        best_ne_raw: str | None = None
        for tok, bio in tagged_ner:
            if bio == "O":
                continue
            if tok == word:
                best_ne_raw = bio
                break
            if best_ne_raw is None and (word in tok or tok in word):
                best_ne_raw = bio
        if best_ne_raw:
            raw_ne_label = best_ne_raw.split("-", 1)[1] if "-" in best_ne_raw else best_ne_raw
            ne = _NE_MAP.get(raw_ne_label.upper())
    except Exception as exc:
        print(f"[NER] error for word={word!r}: {exc}", file=sys.stderr)

    # ── POS ───────────────────────────────────────────────────────────────────
    try:
        from pythainlp.tag import pos_tag
        from pythainlp.tokenize import word_tokenize

        tokens: list[str] = word_tokenize(text, engine="newmm")
        pos_tagged: list[tuple[str, str]] = pos_tag(tokens, engine=POS_ENGINE, corpus=POS_CORPUS)

        partial: tuple[str, str] | None = None
        for tok, ptag in pos_tagged:
            if tok == word:
                raw_pos_label = ptag
                pos = _POS_MAP.get(ptag.upper())
                partial = None
                break
            if partial is None and (word in tok or tok in word):
                partial = (tok, ptag)
        if raw_pos_label is None and partial:
            _, ptag = partial
            raw_pos_label = ptag
            pos = _POS_MAP.get(ptag.upper())
    except Exception as exc:
        print(f"[POS] error for word={word!r}: {exc}", file=sys.stderr)

    # Fallback: named entities are almost always PROPN in Thai
    if ne and pos is None:
        pos = "PROPN"
        raw_pos_label = "PROPN (inferred from NE)"

    found = ne is not None or pos is not None
    return ValidateResponse(
        word=word,
        ne=ne,
        pos=pos,
        confidence=1.0 if found else 0.0,
        raw_label=raw_ne_label,
        pos_raw_label=raw_pos_label,
    )


# ── Routes ────────────────────────────────────────────────────────────────────

@app.get("/health")
def health() -> dict:
    return {
        "ok": _ner_tagger is not None and _pos_ready,
        "ner_engine": NER_ENGINE,
        "pos_engine": f"{POS_ENGINE}/{POS_CORPUS}",
    }


@app.post("/validate", response_model=ValidateResponse)
def validate_post(req: ValidateRequest) -> ValidateResponse:
    return _predict(req.word, req.context)


@app.get("/validate", response_model=ValidateResponse)
def validate_get(
    word: str = Query(..., description="Thai word to classify"),
    context: str = Query("", description="Surrounding sentence for context"),
) -> ValidateResponse:
    return _predict(word, context)


# ── Entry point ───────────────────────────────────────────────────────────────

if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="kham-tnc NER + POS Validator")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=9999)
    parser.add_argument("--reload", action="store_true")
    args = parser.parse_args()

    uvicorn.run(
        "main:app",
        host=args.host,
        port=args.port,
        reload=args.reload,
        log_level="info",
    )
