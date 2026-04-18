---
name: docker-pg
description: Run PostgreSQL extensions inside Docker containers for testing. Use when writing Dockerfiles or entrypoint scripts that start a PostgreSQL server, install a .so extension, and run pg_regress — especially when debugging shared-memory errors, gosu user-switching, socket paths, trust auth, or pg_ctl startup failures.
metadata:
  domain: infrastructure
  triggers: docker postgres extension, pg_regress docker, initdb in container, gosu postgres, dynamic_shared_memory_type, postgresql entrypoint, pg_ctl could not start, trust auth, kham-pg docker test
  role: specialist
---

# docker-pg — PostgreSQL Extensions in Docker

Specialist for running PostgreSQL extensions (`.so` / `cdylib`) inside Docker containers for integration testing with `pg_regress`.

## Base Image

Always use `postgres:17-bookworm` (or `postgres:16-bookworm`).  
The image ships `gosu`, `pg_config`, `initdb`, `pg_ctl`, `pg_isready`, `pg_regress` at `/usr/lib/postgresql/<ver>/bin/`.

```dockerfile
FROM postgres:17-bookworm
```

## Shared Memory — the most common pitfall

PostgreSQL 17+ only supports `posix`, `sysv`, `mmap` — **`none` was removed in PG 17**.

| Value   | Works in Docker? | Notes                                    |
|---------|-----------------|------------------------------------------|
| `posix` | Usually yes      | Default; needs `/dev/shm` ≥ `shared_buffers` |
| `mmap`  | **Always yes**   | Uses files in `$PGDATA`; safest for CI   |
| `sysv`  | Needs `--ipc=host` | Avoid in CI                            |

**Always add to `postgresql.conf` before starting the server:**
```bash
echo "dynamic_shared_memory_type = mmap" >> "$PGDATA/postgresql.conf"
```
OR pass as a startup flag:
```bash
pg_ctl start ... -o "-c dynamic_shared_memory_type=mmap"
```

## User Switching with gosu

The `postgres:17` image runs as `root`. PostgreSQL refuses to run as root.  
Use `gosu postgres <command>` for all DB operations.

```bash
# initdb — must run as postgres
gosu postgres /usr/lib/postgresql/17/bin/initdb -D "$PGDATA" --encoding=UTF8 --locale=C

# pg_ctl — must run as postgres
gosu postgres /usr/lib/postgresql/17/bin/pg_ctl start -D "$PGDATA" -l /tmp/pg.log \
    -o "-p $PGPORT -k $PGSOCKET -c listen_addresses='' -c dynamic_shared_memory_type=mmap"

# pg_isready, createdb, pg_regress — must run as postgres
gosu postgres /usr/lib/postgresql/17/bin/pg_isready -h "$PGSOCKET" -p "$PGPORT" -U postgres -q
```

Directories that postgres must own: `$PGDATA` (700), socket dir (775), log file.

```bash
mkdir -p "$PGDATA" "$PGSOCKET"
chown postgres:postgres "$PGDATA" "$PGSOCKET"
chmod 700 "$PGDATA"
chmod 775 "$PGSOCKET"
touch "$PGLOG" && chown postgres:postgres "$PGLOG"
```

## Socket vs TCP

Prefer Unix socket (`listen_addresses=''`) inside the container — no port exposure needed:

```bash
PGSOCKET=/var/run/postgresql
pg_ctl start -o "-p $PGPORT -k $PGSOCKET -c listen_addresses=''"
pg_isready -h "$PGSOCKET" -p "$PGPORT"
createdb   -h "$PGSOCKET" -p "$PGPORT" mydb
pg_regress --host="$PGSOCKET" --port="$PGPORT" ...
```

## Trust Auth

Add trust rules **after** `initdb` (which overwrites `pg_hba.conf`):

```bash
cat >> "$PGDATA/pg_hba.conf" <<'EOF'
local all all trust
host  all all 127.0.0.1/32 trust
EOF
```

## Error Logging — always capture the PG log

`set -e` exits before you can cat the log. Use `|| { ... }`:

```bash
gosu postgres pg_ctl start ... -l "$PGLOG" || {
    echo "=== PostgreSQL startup log ===" >&2
    cat "$PGLOG" >&2
    exit 1
}
```

## Full Entrypoint Pattern

```bash
#!/usr/bin/env bash
set -euo pipefail

PG_BIN=/usr/lib/postgresql/17/bin
PGDATA=/var/lib/postgresql/17/mytest
PGSOCKET=/var/run/postgresql
PGPORT=15432
PGUSER=postgres
PGLOG=/tmp/pg.log

# Prepare directories (root)
mkdir -p "$PGDATA" "$PGSOCKET"
chown postgres:postgres "$PGDATA" "$PGSOCKET"
chmod 700 "$PGDATA"
chmod 775 "$PGSOCKET"
touch "$PGLOG" && chown postgres:postgres "$PGLOG"

# initdb
gosu postgres "$PG_BIN/initdb" -D "$PGDATA" --encoding=UTF8 --locale=C

# postgresql.conf tweaks
echo "dynamic_shared_memory_type = mmap" >> "$PGDATA/postgresql.conf"

# Trust auth
cat >> "$PGDATA/pg_hba.conf" <<'HBAEOF'
local all all trust
HBAEOF

# Start
gosu postgres "$PG_BIN/pg_ctl" start -D "$PGDATA" -l "$PGLOG" -t 60 \
    -o "-p $PGPORT -k $PGSOCKET -c listen_addresses='' -c dynamic_shared_memory_type=mmap" \
    || { echo "=== PG LOG ===" >&2; cat "$PGLOG" >&2; exit 1; }

# Wait
for i in $(seq 1 30); do
    gosu postgres "$PG_BIN/pg_isready" -h "$PGSOCKET" -p "$PGPORT" -U "$PGUSER" -q \
        && break
    [ "$i" = "30" ] && { cat "$PGLOG"; exit 1; }
done

# Your tests here
gosu postgres "$PG_BIN/createdb" -h "$PGSOCKET" -p "$PGPORT" -U "$PGUSER" testdb
gosu postgres "$PG_BIN/pg_regress" \
    --inputdir=./regress \
    --outputdir=./regress/results \
    --dbname=testdb \
    --host="$PGSOCKET" \
    --port="$PGPORT" \
    --user="$PGUSER" \
    mytest
```

## Installing Extension Files

Copy before entrypoint runs (in Dockerfile), not inside the entrypoint:

```dockerfile
# Build the .so
RUN PG_CONFIG=/usr/lib/postgresql/17/bin/pg_config \
    cargo build -p kham-pg --release

# Install
RUN PG_PKGLIBDIR=$(/usr/lib/postgresql/17/bin/pg_config --pkglibdir) && \
    PG_SHAREDIR=$(/usr/lib/postgresql/17/bin/pg_config --sharedir) && \
    cp target/release/libkham_pg.so "$PG_PKGLIBDIR/kham_pg.so" && \
    cp kham-pg/kham_pg.control "$PG_SHAREDIR/extension/" && \
    cp kham-pg/sql/kham_pg--0.1.0.sql "$PG_SHAREDIR/extension/"
```

## pg_regress Directory Layout

```
regress/
├── sql/
│   └── mytest.sql          # test input
├── expected/
│   └── mytest.out          # expected output (generate with first run, then review)
└── results/                # created at runtime; gitignore this
    ├── output/
    │   └── mytest.out
    └── diff/
        └── mytest.diff     # non-empty = test failure
```

`pg_regress` requires `sql/` and `expected/` under `--inputdir`. Pass test names without `.sql`.

## Generating Expected Output

First run will differ (no expected file yet). To capture:
```bash
# After a run, copy actual output to expected:
docker compose run regress \
    cat /path/to/regress/results/output/mytest.out > regress/expected/mytest.out
```

## docker-compose.yml Pattern

```yaml
services:
  regress:
    build:
      context: ../..          # repo root = cargo workspace
      dockerfile: kham-pg/docker/Dockerfile.test
```

Use `--exit-code-from regress --abort-on-container-exit` to get the test exit code:
```bash
docker compose -f kham-pg/docker/docker-compose.yml up \
    --build \
    --exit-code-from regress \
    --abort-on-container-exit
```

## Known PG Version Differences

| Feature                          | PG 15 | PG 16 | PG 17 |
|----------------------------------|-------|-------|-------|
| `dynamic_shared_memory_type=none`| ✓     | ✓     | ✗ removed |
| `varatt.h` in `postgres.h`       | ✗     | ✗     | ✗ (include explicitly) |
| `ts_token_type` column names     | `tokid,alias,description` | same | same |
