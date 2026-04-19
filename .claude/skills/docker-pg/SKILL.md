---
name: docker-pg
description: Run PostgreSQL extensions inside Docker containers for testing. Use when writing Dockerfiles or entrypoint scripts that start a PostgreSQL server, install a .so extension, and run pg_regress — especially when debugging shared-memory errors, gosu/su-exec user-switching, socket paths, trust auth, pg_ctl startup failures, or symbol export issues.
metadata:
  domain: infrastructure
  triggers: docker postgres extension, pg_regress docker, initdb in container, gosu postgres, su-exec postgres, dynamic_shared_memory_type, postgresql entrypoint, pg_ctl could not start, trust auth, kham-pg docker test, missing magic block, cdylib symbol export
  role: specialist
---

# docker-pg — PostgreSQL Extensions in Docker

Specialist for running PostgreSQL extensions (`.so` / `cdylib`) inside Docker containers for integration testing with `pg_regress`.

## Multi-Stage Build (recommended)

Split builder (Rust + pg dev headers) from runner (PG only). Removes ~1.5 GB Rust toolchain from the test image.

```dockerfile
# Stage 1: builder
FROM debian:bookworm-slim AS builder
RUN apt-get update && apt-get install -y build-essential curl ca-certificates \
    gnupg lsb-release pkg-config \
    && curl -fsSL https://www.postgresql.org/media/keys/ACCC4CF8.asc \
       | gpg --dearmor -o /usr/share/keyrings/pgdg.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/pgdg.gpg] \
       https://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main" \
       > /etc/apt/sources.list.d/pgdg.list \
    && apt-get update && apt-get install -y postgresql-server-dev-17 \
    && rm -rf /var/lib/apt/lists/*
# ... install Rust, cargo build ...

# Stage 2: runner
FROM debian:bookworm-slim AS runner
# install postgresql-17 + postgresql-server-dev-17 (for pg_regress) + gosu
COPY --from=builder /path/to/libkham_pg.so ...
```

**Do NOT use Alpine** for PostgreSQL extensions: Rust's musl targets (`*-unknown-linux-musl`)
are static-only and **do not support `cdylib`**. Use Debian/glibc for both stages.

## Shared Memory — the most common pitfall

PostgreSQL 17+ only supports `posix`, `sysv`, `mmap` — **`none` was removed in PG 17**.

| Value   | Works in Docker? | Notes                                    |
|---------|-----------------|------------------------------------------|
| `mmap`  | **Always yes**   | Uses files in `$PGDATA`; safest for CI   |
| `posix` | Usually yes      | Needs `/dev/shm` ≥ `shared_buffers`     |
| `sysv`  | Needs `--ipc=host` | Avoid in CI                            |

**Always add before starting the server:**
```bash
echo "dynamic_shared_memory_type = mmap" >> "$PGDATA/postgresql.conf"
```

## User Switching

Container runs as root; PostgreSQL refuses to run as root.

| Distro  | Package   | Command                   |
|---------|-----------|---------------------------|
| Debian  | `gosu`    | `gosu postgres <cmd>`     |
| Alpine  | `su-exec` | `su-exec postgres <cmd>`  |

Detect at runtime for portability:
```sh
if command -v su-exec >/dev/null 2>&1; then RUNAS="su-exec postgres"
elif command -v gosu   >/dev/null 2>&1; then RUNAS="gosu postgres"
fi
```

Directories postgres must own: `$PGDATA` (700), socket dir (775), log file.

## pg_config — resolve paths dynamically

```sh
PG_BIN=$(pg_config --bindir)          # /usr/lib/postgresql/17/bin on Debian
PG_PKGLIBDIR=$(pg_config --pkglibdir) # extension .so goes here
PG_EXTDIR=$(pg_config --sharedir)/extension
```

## pg_regress Binary Path

`pg_regress` is **not** in the bin directory. Derive from pgxs:
```sh
PGXS_MK=$(pg_config --pgxs)           # .../pgxs/src/makefiles/pgxs.mk
PG_REGRESS=$(dirname "$(dirname "$PGXS_MK")")/test/regress/pg_regress
# fallback:
[ -x "$PG_REGRESS" ] || PG_REGRESS=$(find /usr/lib/postgresql* -name pg_regress -type f | head -1)
```

## pg_regress --outputdir

`pg_regress` creates `output/` and `results/` **inside** `--outputdir`.
Use `--outputdir=.` (current dir = regress/) so output lands at `regress/output/kham_fts.out`.

```sh
cd /path/to/regress
$RUNAS "$PG_REGRESS" --inputdir=. --outputdir=. --dbname=regression \
    --host="$PGSOCKET" --port="$PGPORT" --user="$PGUSER" kham_fts
```

After a failed run, diffs are at `regression.diffs` (relative to `--outputdir`).

## cdylib Symbol Export (Linux)

Rust's cdylib linker uses `--version-script` that hides all C symbols (`local: *`).
PostgreSQL symbols (`Pg_magic_func`, `kham_start`, `pg_finfo_*`) need to be in the dynamic table.

**Fix: provide a version script in `build.rs`:**
```rust
// build.rs
let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
if target_os == "linux" {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-arg=-Wl,--version-script={manifest_dir}/src/pg_exports.map");
}
```

`src/pg_exports.map`:
```
{
    global:
        Pg_magic_func;
        kham_start; kham_gettoken; kham_end; kham_lextypes;
        pg_finfo_kham_start; pg_finfo_kham_gettoken;
        pg_finfo_kham_end; pg_finfo_kham_lextypes;
    local: *;
};
```

Also add `-fvisibility=default` to `cc::Build` so C symbols compile as `T` (global) before the script runs.

**Verify** with the smoke-test in the builder stage:
```dockerfile
RUN nm -D target/release/libkham_pg.so \
    | grep -E 'Pg_magic_func|kham_start\b' \
    || { echo "ERROR: PG symbols missing"; exit 1; }
```

## Trust Auth

Add **after** `initdb` (which regenerates `pg_hba.conf`):
```sh
printf 'local all all trust\nhost all all 127.0.0.1/32 trust\n' >> "$PGDATA/pg_hba.conf"
```

## Error Logging

`set -e` exits before you can cat the log. Use `|| { ... }`:
```sh
$RUNAS pg_ctl start ... -l "$PGLOG" || { cat "$PGLOG" >&2; exit 1; }
```

## Known PG Version Differences

| Feature                           | PG 15 | PG 16 | PG 17 |
|-----------------------------------|-------|-------|-------|
| `dynamic_shared_memory_type=none` | ✓     | ✓     | ✗ removed |
| `varatt.h` in `postgres.h`        | ✗     | ✗     | ✗ (include explicitly) |
