#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# kham-tnc NER Validator — local setup and run script
#
# Usage:
#   chmod +x scripts/ner_validator/run.sh
#
#   First time (creates venv, downloads model ~750 MB):
#     ./scripts/ner_validator/run.sh
#
#   Subsequent runs:
#     ./scripts/ner_validator/run.sh
#
#   Custom port / host:
#     PORT=9998 ./scripts/ner_validator/run.sh
#     HOST=0.0.0.0 ./scripts/ner_validator/run.sh
#
#   Different NER model:
#     NER_MODEL=pythainlp/thainer-corpus-v2-base-model ./scripts/ner_validator/run.sh
#
#   GPU / Apple Silicon MPS (set device index; -1 = CPU):
#     NER_DEVICE=0 ./scripts/ner_validator/run.sh
#
# After starting, test with:
#   curl http://localhost:9999/health
#   curl -s "http://localhost:9999/validate?word=กรุงเทพ&context=ผมอยู่ที่กรุงเทพ"
#
# Then start kham-tnc pointing at this validator:
#   cargo run -p kham-tnc -- serve --corpus tnc_test.sqlite --validator-url http://127.0.0.1:9999
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_DIR="$SCRIPT_DIR/.venv"
PORT="${PORT:-9999}"
HOST="${HOST:-127.0.0.1}"
export NER_MODEL="${NER_MODEL:-airesearch/wangchanberta-base-att-spm-uncased-finetuned-thainer}"
export NER_DEVICE="${NER_DEVICE:--1}"

# ── Python version check ─────────────────────────────────────────────────────
PYTHON=$(command -v python3 || command -v python || true)
if [ -z "$PYTHON" ]; then
  echo "ERROR: Python 3 not found. Install Python 3.9+ first." >&2
  exit 1
fi

PY_VERSION=$("$PYTHON" -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')")
PY_MAJOR=$("$PYTHON" -c "import sys; print(sys.version_info.major)")
PY_MINOR=$("$PYTHON" -c "import sys; print(sys.version_info.minor)")

if [ "$PY_MAJOR" -lt 3 ] || { [ "$PY_MAJOR" -eq 3 ] && [ "$PY_MINOR" -lt 9 ]; }; then
  echo "ERROR: Python 3.9+ required (found $PY_VERSION)" >&2
  exit 1
fi
echo "Python $PY_VERSION — OK"

# ── Virtual environment ───────────────────────────────────────────────────────
if [ ! -d "$VENV_DIR" ]; then
  echo "Creating virtual environment at $VENV_DIR …"
  "$PYTHON" -m venv "$VENV_DIR"
fi

# shellcheck disable=SC1091
source "$VENV_DIR/bin/activate"

# ── Install / upgrade dependencies ──────────────────────────────────────────
if [ ! -f "$VENV_DIR/.installed" ] || [ "$SCRIPT_DIR/requirements.txt" -nt "$VENV_DIR/.installed" ]; then
  echo "Installing dependencies (first run: ~750 MB download including model) …"
  pip install --quiet --upgrade pip
  pip install --quiet -r "$SCRIPT_DIR/requirements.txt"
  touch "$VENV_DIR/.installed"
  echo "Dependencies installed."
fi

# ── Pre-download model (cached in HuggingFace cache after first download) ────
echo "Checking model: $NER_MODEL"
"$PYTHON" - <<'PYEOF'
import os, sys
model = os.environ["NER_MODEL"]
try:
    from transformers import AutoTokenizer, AutoModelForTokenClassification
    print(f"  Downloading / loading tokenizer …")
    AutoTokenizer.from_pretrained(model)
    print(f"  Downloading / loading model weights …")
    AutoModelForTokenClassification.from_pretrained(model)
    print(f"  Model ready: {model}")
except Exception as e:
    print(f"  WARNING: could not pre-load model: {e}", file=sys.stderr)
    print("  The model will be downloaded on first request instead.")
PYEOF

# ── Start the server ─────────────────────────────────────────────────────────
echo ""
echo "──────────────────────────────────────────────────────────────"
echo " kham-tnc NER Validator"
echo " Model  : $NER_MODEL"
echo " Device : $NER_DEVICE  (-1 = CPU)"
echo " URL    : http://$HOST:$PORT"
echo ""
echo " Health check : curl http://$HOST:$PORT/health"
echo " Test         : curl -s \"http://$HOST:$PORT/validate?word=กรุงเทพ&context=ผมอยู่ที่กรุงเทพ\""
echo ""
echo " Start kham-tnc with:"
echo "   cargo run -p kham-tnc -- serve --corpus tnc_test.sqlite \\"
echo "     --validator-url http://$HOST:$PORT"
echo "──────────────────────────────────────────────────────────────"
echo ""

cd "$SCRIPT_DIR"
exec uvicorn main:app --host "$HOST" --port "$PORT" --log-level info
