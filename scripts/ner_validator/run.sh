#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# kham-tnc NER Validator — local setup and run script
#
# Usage:
#   chmod +x scripts/ner_validator/run.sh
#
#   First time (creates venv, downloads model ~600 MB total):
#     ./scripts/ner_validator/run.sh
#
#   Subsequent runs (fast, model already cached by pythainlp):
#     ./scripts/ner_validator/run.sh
#
#   Custom port / host:
#     PORT=9998 ./scripts/ner_validator/run.sh
#     HOST=0.0.0.0 ./scripts/ner_validator/run.sh
#
#   Different NER engine (thainer or thainer-v2):
#     NER_ENGINE=thainer-v2 ./scripts/ner_validator/run.sh
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
# thainer-v2 = WangchanBERTa-based (no pycrfsuite required, works on Python 3.14+)
# thainer    = CRF-based (requires pycrfsuite — not available for Python 3.14)
export NER_ENGINE="${NER_ENGINE:-thainer-v2}"

# ── Python version check ─────────────────────────────────────────────────────
PYTHON=$(command -v python3 || command -v python || true)
if [ -z "$PYTHON" ]; then
  echo "ERROR: Python 3 not found. Install Python 3.9+ first." >&2
  exit 1
fi

PY_MAJOR=$("$PYTHON" -c "import sys; print(sys.version_info.major)")
PY_MINOR=$("$PYTHON" -c "import sys; print(sys.version_info.minor)")
PY_VERSION="$PY_MAJOR.$PY_MINOR"

if [ "$PY_MAJOR" -lt 3 ] || { [ "$PY_MAJOR" -eq 3 ] && [ "$PY_MINOR" -lt 9 ]; }; then
  echo "ERROR: Python 3.9+ required (found $PY_VERSION)" >&2
  exit 1
fi
echo "Python $PY_VERSION — OK"

# ── Stale venv detection: delete if requirements.txt changed ─────────────────
# (This handles the case where requirements.txt was updated after .venv was created)
if [ -d "$VENV_DIR" ] && [ -f "$VENV_DIR/.installed" ]; then
  if [ "$SCRIPT_DIR/requirements.txt" -nt "$VENV_DIR/.installed" ]; then
    echo "requirements.txt changed — removing old virtual environment …"
    rm -rf "$VENV_DIR"
  fi
fi

# ── Virtual environment ───────────────────────────────────────────────────────
if [ ! -d "$VENV_DIR" ]; then
  echo "Creating virtual environment at $VENV_DIR …"
  "$PYTHON" -m venv "$VENV_DIR"
fi

# shellcheck disable=SC1091
source "$VENV_DIR/bin/activate"

# ── Install / upgrade dependencies ──────────────────────────────────────────
if [ ! -f "$VENV_DIR/.installed" ]; then
  echo "Installing dependencies …"
  echo "(First run downloads PyTorch + pythainlp models ~600 MB — be patient)"
  pip install --quiet --no-cache-dir --upgrade pip
  pip install --quiet --no-cache-dir -r "$SCRIPT_DIR/requirements.txt"
  touch "$VENV_DIR/.installed"
  echo "Dependencies installed."
fi

# ── Pre-download pythainlp NER model ─────────────────────────────────────────
echo "Checking pythainlp NER model (engine=$NER_ENGINE) …"
"$PYTHON" - <<PYEOF
import os, sys
engine = os.environ["NER_ENGINE"]
try:
    from pythainlp.tag import NER as ThaiNER
    tagger = ThaiNER(engine=engine)
    tagger.tag("ทดสอบ")  # triggers download if needed
    print(f"  Model ready: {engine}")
except Exception as e:
    print(f"  WARNING: {e}", file=sys.stderr)
    print("  Model will be downloaded on first request.")
PYEOF

# ── Start the server ─────────────────────────────────────────────────────────
echo ""
echo "──────────────────────────────────────────────────────────────"
echo " kham-tnc NER Validator"
echo " Engine : $NER_ENGINE  (thainer-v2 = WangchanBERTa; thainer = CRF/Python≤3.13 only)"
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
