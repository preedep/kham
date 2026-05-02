# kham-pg Makefile
#
# Targets:
#   build          — compile the Rust cdylib against the host pg_config
#   install        — build + copy .so / control / SQL files to the host PG install
#   regress        — run pg_regress integration tests inside Docker (default PG 17)
#   regress-matrix — run pg_regress against PG 14, 15, 16, 17, 18 in sequence
#   bench          — run throughput benchmark inside Docker (PG 17); reports ops/s and ms/op
#   dist           — produce kham_pg-$(EXT_VERSION).zip for PGXN upload
#   clean          — remove Cargo build artefacts and dist directory
#
# Prerequisites (for `regress` / `regress-matrix`): Docker with BuildKit support.
# Set PG_CONFIG to override which pg_config is used for `build` and `install`.
# Set PG_VERSION to override the PostgreSQL major version used in Docker (default 17).

PG_CONFIG      ?= pg_config
CARGO          ?= cargo
DOCKER_COMPOSE ?= docker compose
PG_VERSION     ?= 17
PG_VERSIONS    := 14 15 16 17 18

EXT_VERSION       := 0.8.0
KHAM_CORE_VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' ../Cargo.toml | head -1)
DIST_NAME         := kham_pg-$(EXT_VERSION)

PG_PKGLIBDIR    = $(shell $(PG_CONFIG) --pkglibdir)
PG_SHAREDIR     = $(shell $(PG_CONFIG) --sharedir)
EXT_DIR         = $(PG_SHAREDIR)/extension

UNAME_S        := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
  LIBNAME      := libkham_pg.dylib
else
  LIBNAME      := libkham_pg.so
endif
RELEASE_LIB    := ../target/release/$(LIBNAME)

.PHONY: build install regress regress-matrix regress-update-expected bench dist print-version clean

# ── build ─────────────────────────────────────────────────────────────────────

build:
	PG_CONFIG=$(PG_CONFIG) $(CARGO) build -p kham-pg --release

# ── install ───────────────────────────────────────────────────────────────────

install: build
	install -m 755 $(RELEASE_LIB) $(PG_PKGLIBDIR)/kham_pg.so
	install -m 644 kham_pg.control $(EXT_DIR)/
	install -m 644 sql/kham_pg--$(EXT_VERSION).sql $(EXT_DIR)/
	install -m 644 sql/kham_pg--0.7.0--0.8.0.sql $(EXT_DIR)/
	@echo "Installed kham_pg $(EXT_VERSION) into $(EXT_DIR)"

# ── regress (Docker) ──────────────────────────────────────────────────────────

regress:
	PG_VERSION=$(PG_VERSION) $(DOCKER_COMPOSE) -f docker/docker-compose.yml up \
	    --build \
	    --exit-code-from regress \
	    --abort-on-container-exit

# ── regress-matrix — run regress against all supported PG versions ────────────

regress-matrix:
	@failed=""; \
	for v in $(PG_VERSIONS); do \
	    printf '\n=== kham-pg regress: PG %s ===\n' "$$v"; \
	    PG_VERSION=$$v $(DOCKER_COMPOSE) -f docker/docker-compose.yml up \
	        --build \
	        --exit-code-from regress \
	        --abort-on-container-exit \
	        || failed="$$failed $$v"; \
	    PG_VERSION=$$v $(DOCKER_COMPOSE) -f docker/docker-compose.yml down \
	        --rmi local 2>/dev/null || true; \
	done; \
	echo ""; \
	if [ -n "$$failed" ]; then \
	    printf 'FAILED on PG versions:%s\n' "$$failed"; exit 1; \
	fi; \
	printf 'All passed: PG %s\n' "$(PG_VERSIONS)"

# Regenerate expected output files from the current Docker run.
# After running this, review regress/results/output/kham_fts.out and, if
# correct, copy it to regress/expected/kham_fts.out.
regress-update-expected:
	PG_VERSION=$(PG_VERSION) $(DOCKER_COMPOSE) -f docker/docker-compose.yml up \
	    --build \
	    --exit-code-from regress \
	    --abort-on-container-exit || true
	@echo ""
	@echo "Actual output is in the container at /kham/kham-pg/regress/results/output/kham_fts.out"
	@echo "Run: docker compose -f docker/docker-compose.yml run regress cat /kham/kham-pg/regress/results/output/kham_fts.out"
	@echo "     > regress/expected/kham_fts.out"

# ── bench (Docker) ───────────────────────────────────────────────────────────

bench:
	$(DOCKER_COMPOSE) -f bench/docker-compose.yml up \
	    --build \
	    --exit-code-from bench \
	    --abort-on-container-exit \
	    --remove-orphans

# ── dist — PGXN distribution zip ─────────────────────────────────────────────
#
# Produces $(DIST_NAME).zip with a standalone Cargo.toml that references
# kham-core from crates.io (not the workspace path dep).  Users unzip and run
# `make install` — Cargo fetches kham-core automatically.

dist:
	rm -rf $(DIST_NAME) $(DIST_NAME).zip
	mkdir -p $(DIST_NAME)/sql $(DIST_NAME)/src
	# Control and SQL
	cp META.json kham_pg.control $(DIST_NAME)/
	cp README.pgxn.md $(DIST_NAME)/README.md
	cp sql/kham_pg--$(EXT_VERSION).sql $(DIST_NAME)/sql/
	cp sql/kham_pg--0.1.0--0.2.0.sql $(DIST_NAME)/sql/
	cp sql/kham_pg--0.1.3--0.2.0.sql $(DIST_NAME)/sql/
	cp sql/kham_pg--0.2.0--0.3.0.sql $(DIST_NAME)/sql/
	cp sql/kham_pg--0.3.0--0.6.0.sql $(DIST_NAME)/sql/
	cp sql/kham_pg--0.6.0--0.7.0.sql $(DIST_NAME)/sql/
	cp sql/kham_pg--0.7.0--0.8.0.sql $(DIST_NAME)/sql/
	# Source
	cp src/lib.rs src/shim.c $(DIST_NAME)/src/
	cp build.rs $(DIST_NAME)/
	# Standalone Cargo.toml (registry dep, no workspace keys)
	printf '[package]\n\
name = "kham-pg"\n\
version = "$(EXT_VERSION)"\n\
edition = "2021"\n\
rust-version = "1.85"\n\
license = "MIT OR Apache-2.0"\n\
description = "PostgreSQL text-search extension for Thai (kham-pg)"\n\
repository = "https://github.com/preedep/kham"\n\
\n\
[lib]\n\
crate-type = ["cdylib"]\n\
\n\
[dependencies]\n\
kham-core = { version = "$(KHAM_CORE_VERSION)", features = ["std"] }\n\
\n\
[build-dependencies]\n\
cc = "1"\n' > $(DIST_NAME)/Cargo.toml
	# Makefile (standalone — no workspace cargo flags)
	sed 's/-p kham-pg//' Makefile > $(DIST_NAME)/Makefile
	zip -r $(DIST_NAME).zip $(DIST_NAME)
	rm -rf $(DIST_NAME)
	@echo "Created $(DIST_NAME).zip — ready for PGXN upload"

# ── print-version — used by CI to get the zip filename ───────────────────────

print-version:
	@echo $(EXT_VERSION)

# ── clean ─────────────────────────────────────────────────────────────────────

clean:
	$(CARGO) clean -p kham-pg
	$(DOCKER_COMPOSE) -f docker/docker-compose.yml down --rmi local 2>/dev/null || true
	$(DOCKER_COMPOSE) -f bench/docker-compose.yml down --rmi local 2>/dev/null || true
	rm -rf kham_pg-*.zip kham_pg-*/
