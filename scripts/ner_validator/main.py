#!/usr/bin/env python3
"""
kham-tnc NER Validator sidecar.

Uses pythainlp's ThaiNER (WangchanBERTa-based) for Named Entity Recognition.
pythainlp manages model downloads via its own corpus manager — no HuggingFace
login required.

Endpoints:
  GET  /health                   — liveness check
  POST /validate                 — predict NE tag for a word in context
  GET  /validate?word=X&context= — same, via GET (browser-friendly)
"""

import os

import uvicorn
from fastapi import FastAPI, Query
from pydantic import BaseModel

# ── Config ──────────────────────────────────────────────────────────────────
# NER engine: "thainer" (default) or "thainer-v2"
# pythainlp will download the model automatically on first use (~450 MB)
NER_ENGINE: str = os.getenv("NER_ENGINE", "thainer")

# ── Label mapping → kham-core NE tags ────────────────────────────────────────
_LABEL_MAP: dict[str, str] = {
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

app = FastAPI(title="kham-tnc NER Validator", version="1.1.0")
_tagger = None  # loaded on startup


@app.on_event("startup")
async def _load_model() -> None:
    global _tagger
    print(f"Loading ThaiNER model via pythainlp (engine={NER_ENGINE}) …")
    from pythainlp.tag import NER as ThaiNER  # noqa: PLC0415

    _tagger = ThaiNER(engine=NER_ENGINE)
    # Warm-up: triggers model download if not cached
    _tagger.tag("ทดสอบ")
    print("Model ready.")


# ── Schemas ──────────────────────────────────────────────────────────────────

class ValidateRequest(BaseModel):
    word: str
    context: str = ""


class ValidateResponse(BaseModel):
    word: str
    ne: str | None        # kham-core NE tag or None
    confidence: float     # 1.0 when found; pythainlp NER doesn't expose scores
    raw_label: str | None # original BIO label (e.g. "B-LOCATION")


# ── Prediction logic ──────────────────────────────────────────────────────────

def _predict(word: str, context: str) -> ValidateResponse:
    """
    Tag `context` (or `word` alone if no context) and find the NE label
    for any token that overlaps with `word`.
    """
    text = context.strip() or word
    # tagged: list of (token_word, bio_ne_label), e.g. [('กรุงเทพ', 'B-LOCATION'), ...]
    tagged: list[tuple[str, str]] = _tagger.tag(text)

    best_raw: str | None = None
    for tok, bio in tagged:
        if bio == "O":
            continue
        # Overlap check: word contains token or token contains word
        if word in tok or tok in word:
            best_raw = bio
            break

    if best_raw is None:
        return ValidateResponse(word=word, ne=None, confidence=0.0, raw_label=None)

    # Strip BIO prefix: "B-LOCATION" → "LOCATION"
    raw_label = best_raw.split("-", 1)[1] if "-" in best_raw else best_raw
    ne = _LABEL_MAP.get(raw_label.upper())

    return ValidateResponse(
        word=word,
        ne=ne,
        confidence=1.0,
        raw_label=best_raw,
    )


# ── Routes ───────────────────────────────────────────────────────────────────

@app.get("/health")
def health() -> dict:
    return {"ok": _tagger is not None, "engine": NER_ENGINE}


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

    parser = argparse.ArgumentParser(description="kham-tnc NER Validator")
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
