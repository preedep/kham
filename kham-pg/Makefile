# kham-pg Makefile
#
# Targets:
#   build    — compile the Rust cdylib against the host pg_config
#   install  — build + copy .so / control / SQL files to the host PG install
#   regress  — run pg_regress integration tests inside Docker (PG 17)
#   clean    — remove Cargo build artefacts
#
# Prerequisites (for `regress`): Docker with BuildKit support.
# Set PG_CONFIG to override which pg_config is used for `build` and `install`.

PG_CONFIG     ?= pg_config
CARGO         ?= cargo
DOCKER_COMPOSE ?= docker compose

PG_PKGLIBDIR  := $(shell $(PG_CONFIG) --pkglibdir)
PG_SHAREDIR   := $(shell $(PG_CONFIG) --sharedir)
EXT_DIR       := $(PG_SHAREDIR)/extension

LIBNAME       := libkham_pg.so
RELEASE_LIB   := ../target/release/$(LIBNAME)

.PHONY: build install regress regress-update-expected clean

# ── build ─────────────────────────────────────────────────────────────────────

build:
	PG_CONFIG=$(PG_CONFIG) $(CARGO) build -p kham-pg --release

# ── install ───────────────────────────────────────────────────────────────────

install: build
	install -m 755 $(RELEASE_LIB) $(PG_PKGLIBDIR)/kham_pg.so
	install -m 644 kham_pg.control $(EXT_DIR)/
	install -m 644 sql/kham_pg--0.1.0.sql $(EXT_DIR)/
	@echo "Installed kham_pg 0.1.0 into $(EXT_DIR)"

# ── regress (Docker) ──────────────────────────────────────────────────────────

regress:
	$(DOCKER_COMPOSE) -f docker/docker-compose.yml up \
	    --build \
	    --exit-code-from regress \
	    --abort-on-container-exit

# Regenerate expected output files from the current Docker run.
# After running this, review regress/results/output/kham_fts.out and, if
# correct, copy it to regress/expected/kham_fts.out.
regress-update-expected:
	$(DOCKER_COMPOSE) -f docker/docker-compose.yml up \
	    --build \
	    --exit-code-from regress \
	    --abort-on-container-exit || true
	@echo ""
	@echo "Actual output is in the container at /kham/kham-pg/regress/results/output/kham_fts.out"
	@echo "Run: docker compose -f docker/docker-compose.yml run regress cat /kham/kham-pg/regress/results/output/kham_fts.out"
	@echo "     > regress/expected/kham_fts.out"

# ── clean ─────────────────────────────────────────────────────────────────────

clean:
	$(CARGO) clean -p kham-pg
	$(DOCKER_COMPOSE) -f docker/docker-compose.yml down --rmi local 2>/dev/null || true
