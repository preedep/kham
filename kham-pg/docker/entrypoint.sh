#!/usr/bin/env bash
# entrypoint.sh — initialise PostgreSQL 17, install kham-pg, run pg_regress.
#
# Runs as root inside the postgres:17-bookworm image.
# Database operations are delegated to the unprivileged `postgres` user via gosu.
set -euo pipefail

PG_BIN=/usr/lib/postgresql/17/bin
PGDATA=/var/lib/postgresql/17/kham_test
PGSOCKET=/var/run/postgresql
PGPORT=15432
PGUSER=postgres
PGLOG=/tmp/pg_kham.log

# ── Prepare directories ───────────────────────────────────────────────────────
mkdir -p "$PGDATA"
chown postgres:postgres "$PGDATA"
chmod 700 "$PGDATA"

mkdir -p "$PGSOCKET"
chown postgres:postgres "$PGSOCKET"
chmod 775 "$PGSOCKET"

touch "$PGLOG"
chown postgres:postgres "$PGLOG"

# ── Initialise cluster ───────────────────────────────────────────────────────
echo "[entrypoint] Running initdb..."
gosu postgres "$PG_BIN/initdb" -D "$PGDATA" --encoding=UTF8 --locale=C

# Use mmap for shared memory — works in restricted Docker containers without
# requiring the host to configure POSIX shared memory limits.
echo "dynamic_shared_memory_type = mmap" >> "$PGDATA/postgresql.conf"

# Trust auth for local socket
cat >> "$PGDATA/pg_hba.conf" <<'HBAEOF'
local all all trust
host  all all 127.0.0.1/32 trust
HBAEOF

# ── Start server ─────────────────────────────────────────────────────────────
echo "[entrypoint] Starting PostgreSQL..."
gosu postgres "$PG_BIN/pg_ctl" start -D "$PGDATA" -l "$PGLOG" -t 60 \
    -o "-p $PGPORT -k $PGSOCKET -c listen_addresses='' -c dynamic_shared_memory_type=mmap" \
    || {
        echo "[entrypoint] ERROR: PostgreSQL failed to start. Log:"
        cat "$PGLOG"
        exit 1
    }

# Wait for server to accept connections
for i in $(seq 1 30); do
    if gosu postgres "$PG_BIN/pg_isready" -h "$PGSOCKET" -p "$PGPORT" -U "$PGUSER" -q 2>/dev/null; then
        break
    fi
    if [ "$i" = "30" ]; then
        echo "[entrypoint] ERROR: server not ready after 30s. Log:"
        cat "$PGLOG"
        exit 1
    fi
done
echo "[entrypoint] PostgreSQL 17 ready"

# ── Create regression database ───────────────────────────────────────────────
gosu postgres "$PG_BIN/createdb" -h "$PGSOCKET" -p "$PGPORT" -U "$PGUSER" regression

# ── Run pg_regress ───────────────────────────────────────────────────────────
cd /kham/kham-pg/regress
mkdir -p results
chown -R postgres:postgres results

gosu postgres "$PG_BIN/pg_regress" \
    --inputdir=. \
    --outputdir=results \
    --dbname=regression \
    --host="$PGSOCKET" \
    --port="$PGPORT" \
    --user="$PGUSER" \
    kham_fts

STATUS=$?

if [ $STATUS -ne 0 ]; then
    echo ""
    echo "=== REGRESSION DIFFS ==="
    find results -name "*.diff" -exec cat {} \; 2>/dev/null || true
fi

exit $STATUS
