#!/usr/bin/env python3
"""
kham-tnc NER Validator sidecar.

Wraps WangchanBERTa (ThaiNER) and exposes a simple REST API so that
kham-tnc can suggest NE tags for Thai words in context.

Endpoints:
  GET  /health                   — liveness check
  POST /validate                 — predict NE tag for a word in context
  GET  /validate?word=X&context= — same, via GET (browser-friendly)
"""

import os
import re

import uvicorn
from fastapi import FastAPI, Query
from pydantic import BaseModel
from transformers import pipeline

# ── Config ──────────────────────────────────────────────────────────────────
MODEL_NAME = os.getenv(
    "NER_MODEL",
    "airesearch/wangchanberta-base-att-spm-uncased-finetuned-thainer",
)
DEVICE = int(os.getenv("NER_DEVICE", "-1"))  # -1 = CPU; 0 = first GPU / MPS

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
    "CARDINAL": "MISC",
    "MISC": "MISC",
}

app = FastAPI(title="kham-tnc NER Validator", version="1.0.0")
_ner: object = None  # loaded on startup


@app.on_event("startup")
async def _load_model() -> None:
    global _ner
    print(f"Loading NER model: {MODEL_NAME}  (device={DEVICE})")
    _ner = pipeline(
        "token-classification",
        model=MODEL_NAME,
        aggregation_strategy="simple",
        device=DEVICE,
    )
    print("Model ready.")


# ── Schemas ──────────────────────────────────────────────────────────────────

class ValidateRequest(BaseModel):
    word: str
    context: str = ""


class ValidateResponse(BaseModel):
    word: str
    ne: str | None       # kham-core tag (PERSON / PLACE / ORG / DATE / …) or None
    confidence: float
    raw_label: str | None  # original model label before mapping


# ── Helpers ──────────────────────────────────────────────────────────────────

def _predict(word: str, context: str) -> ValidateResponse:
    """Run NER and find the entity that best matches `word` in `context`."""
    text = context.strip() or word
    entities = _ner(text)

    word_start = text.find(word)
    if word_start == -1:
        # Word not literally present in context — run NER on the word alone
        entities = _ner(word)
        text = word
        word_start = 0
    word_end = word_start + len(word)

    # Find the entity with the greatest overlap with the word span
    best: dict | None = None
    for ent in entities:
        e_start: int = ent.get("start", 0)
        e_end: int = ent.get("end", len(text))
        overlap = min(word_end, e_end) - max(word_start, e_start)
        if overlap > 0:
            if best is None or ent["score"] > best["score"]:
                best = ent

    if best is None:
        return ValidateResponse(word=word, ne=None, confidence=0.0, raw_label=None)

    raw = best["entity_group"]
    ne = _LABEL_MAP.get(raw.upper())
    return ValidateResponse(
        word=word,
        ne=ne,
        confidence=round(float(best["score"]), 4),
        raw_label=raw,
    )


# ── Routes ───────────────────────────────────────────────────────────────────

@app.get("/health")
def health() -> dict:
    return {"ok": _ner is not None, "model": MODEL_NAME}


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
