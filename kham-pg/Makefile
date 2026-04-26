# kham-pg Makefile
#
# Targets:
#   build    — compile the Rust cdylib against the host pg_config
#   install  — build + copy .so / control / SQL files to the host PG install
#   regress  — run pg_regress integration tests inside Docker (PG 17)
#   dist     — produce kham_pg-$(EXT_VERSION).zip for PGXN upload
#   clean    — remove Cargo build artefacts and dist directory
#
# Prerequisites (for `regress`): Docker with BuildKit support.
# Set PG_CONFIG to override which pg_config is used for `build` and `install`.

PG_CONFIG      ?= pg_config
CARGO          ?= cargo
DOCKER_COMPOSE ?= docker compose

EXT_VERSION    := 0.3.0
DIST_NAME      := kham_pg-$(EXT_VERSION)

PG_PKGLIBDIR   := $(shell $(PG_CONFIG) --pkglibdir)
PG_SHAREDIR    := $(shell $(PG_CONFIG) --sharedir)
EXT_DIR        := $(PG_SHAREDIR)/extension

LIBNAME        := libkham_pg.so
RELEASE_LIB    := ../target/release/$(LIBNAME)

.PHONY: build install regress regress-update-expected dist clean

# ── build ─────────────────────────────────────────────────────────────────────

build:
	PG_CONFIG=$(PG_CONFIG) $(CARGO) build -p kham-pg --release

# ── install ───────────────────────────────────────────────────────────────────

install: build
	install -m 755 $(RELEASE_LIB) $(PG_PKGLIBDIR)/kham_pg.so
	install -m 644 kham_pg.control $(EXT_DIR)/
	install -m 644 sql/kham_pg--$(EXT_VERSION).sql $(EXT_DIR)/
	@echo "Installed kham_pg $(EXT_VERSION) into $(EXT_DIR)"

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
	cp sql/kham_pg--$(EXT_VERSION).sql $(DIST_NAME)/sql/
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
kham-core = { version = "$(EXT_VERSION)", features = ["std"] }\n\
\n\
[build-dependencies]\n\
cc = "1"\n' > $(DIST_NAME)/Cargo.toml
	# Makefile (standalone — no workspace cargo flags)
	sed 's/-p kham-pg//' Makefile > $(DIST_NAME)/Makefile
	zip -r $(DIST_NAME).zip $(DIST_NAME)
	rm -rf $(DIST_NAME)
	@echo "Created $(DIST_NAME).zip — ready for PGXN upload"

# ── clean ─────────────────────────────────────────────────────────────────────

clean:
	$(CARGO) clean -p kham-pg
	$(DOCKER_COMPOSE) -f docker/docker-compose.yml down --rmi local 2>/dev/null || true
	rm -rf kham_pg-*.zip kham_pg-*/
