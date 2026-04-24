#!/usr/bin/env bash
# deploy.sh — publish kham packages to crates.io, npm, and PyPI
#
# Usage:
#   ./scripts/deploy.sh [modules...]   # deploy specific modules
#   ./scripts/deploy.sh --all          # deploy everything
#   ./scripts/deploy.sh --dry-run core capi
#
# Modules (deployed in dependency order):
#   core   → crates.io (kham-core)
#   capi   → crates.io (kham-capi, requires core on crates.io)
#   cli    → crates.io (kham-cli,  requires core on crates.io)
#   wasm   → npm       (kham-wasm)
#   python → PyPI      (kham-python / kham on PyPI)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── colours ────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BLUE='\033[0;34m'; BOLD='\033[1m'; RESET='\033[0m'

info()    { echo -e "${BLUE}[info]${RESET}  $*"; }
success() { echo -e "${GREEN}[ok]${RESET}    $*"; }
warn()    { echo -e "${YELLOW}[warn]${RESET}  $*"; }
error()   { echo -e "${RED}[error]${RESET} $*" >&2; }
die()     { error "$*"; exit 1; }
step()    { echo -e "\n${BOLD}══ $* ${RESET}"; }

# ── argument parsing ────────────────────────────────────────────────────────
DRY_RUN=false
DEPLOY_CORE=false; DEPLOY_CAPI=false; DEPLOY_CLI=false
DEPLOY_WASM=false; DEPLOY_PYTHON=false

usage() {
  echo "Usage: $0 [--dry-run] [--all | core capi cli wasm python]"
  echo ""
  echo "Modules:"
  echo "  core    kham-core  → crates.io"
  echo "  capi    kham-capi  → crates.io"
  echo "  cli     kham-cli   → crates.io"
  echo "  wasm    kham-wasm  → npm"
  echo "  python  kham       → PyPI"
  exit 0
}

[[ $# -eq 0 ]] && usage

for arg in "$@"; do
  case "$arg" in
    --all)      DEPLOY_CORE=true; DEPLOY_CAPI=true; DEPLOY_CLI=true
                DEPLOY_WASM=true; DEPLOY_PYTHON=true ;;
    --dry-run)  DRY_RUN=true ;;
    core)       DEPLOY_CORE=true ;;
    capi)       DEPLOY_CAPI=true ;;
    cli)        DEPLOY_CLI=true ;;
    wasm)       DEPLOY_WASM=true ;;
    python)     DEPLOY_PYTHON=true ;;
    --help|-h)  usage ;;
    *)          die "Unknown argument: $arg" ;;
  esac
done

$DRY_RUN && warn "DRY RUN — no packages will be uploaded"

# ── preflight checks ────────────────────────────────────────────────────────
step "Preflight checks"
cd "$ROOT"

# tests
info "Running cargo test..."
cargo test --workspace --exclude kham-python --exclude kham-wasm --exclude kham-pg 2>&1 | tail -3
success "Tests pass"

# fmt
info "Checking formatting..."
cargo fmt --all -- --check || die "Run 'cargo fmt --all' first"
success "Formatting OK"

# clippy
info "Running clippy..."
cargo clippy --workspace --exclude kham-python --exclude kham-wasm --exclude kham-pg \
  --all-targets -- -D warnings 2>&1 | grep -E "^error" && die "Clippy errors" || true
success "Clippy OK"

# tool availability
($DEPLOY_WASM)   && ! command -v wasm-pack &>/dev/null && die "wasm-pack not found — run: cargo install wasm-pack"
($DEPLOY_PYTHON) && ! command -v maturin   &>/dev/null && die "maturin not found — run: pip install maturin"
($DEPLOY_WASM)   && ! npm whoami &>/dev/null 2>&1      && die "Not logged in to npm — run: npm login"
if $DEPLOY_PYTHON; then
  [[ -z "${MATURIN_PYPI_TOKEN:-}" ]] && die "MATURIN_PYPI_TOKEN is not set"
fi

# ── helper: cargo publish ───────────────────────────────────────────────────
publish_crate() {
  local crate="$1" package="$2"
  step "Publishing $crate → crates.io"
  if $DRY_RUN; then
    cargo publish -p "$package" --dry-run
    success "$crate dry-run OK"
  else
    cargo publish -p "$package"
    success "$crate published"
    info "Waiting 30s for crates.io index to update..."
    sleep 30
  fi
}

# ── deploy in dependency order ──────────────────────────────────────────────

# 1. core (must come before capi / cli)
if $DEPLOY_CORE; then
  publish_crate "kham-core" "kham-core"
fi

# 2. capi (depends on kham-core on crates.io)
if $DEPLOY_CAPI; then
  ! $DEPLOY_CORE && warn "Deploying kham-capi without kham-core — ensure kham-core is already live on crates.io"
  publish_crate "kham-capi" "kham-capi"
fi

# 3. cli (depends on kham-core on crates.io)
if $DEPLOY_CLI; then
  ! $DEPLOY_CORE && warn "Deploying kham-cli without kham-core — ensure kham-core is already live on crates.io"
  publish_crate "kham-cli" "kham-cli"
fi

# 4. wasm → npm
if $DEPLOY_WASM; then
  step "Building kham-wasm → npm"
  wasm-pack build kham-wasm --target web --release
  if $DRY_RUN; then
    info "DRY RUN — skipping npm publish (pkg ready at kham-wasm/pkg/)"
  else
    npm publish kham-wasm/pkg/
    success "kham-wasm published to npm"
  fi
fi

# 5. python → PyPI
if $DEPLOY_PYTHON; then
  step "Building kham-python → PyPI"
  cd "$ROOT/kham-python"
  if $DRY_RUN; then
    maturin build --release
    info "DRY RUN — wheel built at $ROOT/target/wheels/, skipping upload"
  else
    maturin build --release
    maturin upload "$ROOT"/target/wheels/kham-"$(grep '^version' "$ROOT/Cargo.toml" | head -1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')"*.whl
    success "kham published to PyPI"
  fi
  cd "$ROOT"
fi

# ── done ────────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}Release complete.${RESET}"
$DRY_RUN && echo -e "${YELLOW}(dry run — nothing was actually published)${RESET}"
