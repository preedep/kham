#!/usr/bin/env sh
# entrypoint.sh — initialise PostgreSQL, install kham_pg, run bench.sql.
#
# Reports ops/s and ms/op for each FTS operation via RAISE NOTICE.
set -eu

# ── Detect privilege-drop helper ─────────────────────────────────────────────
if command -v su-exec >/dev/null 2>&1; then
    RUNAS="su-exec postgres"
elif command -v gosu >/dev/null 2>&1; then
    RUNAS="gosu postgres"
else
    echo "[bench] ERROR: neither su-exec nor gosu found" >&2
    exit 1
fi

# ── Resolve PG paths ──────────────────────────────────────────────────────────
PG_BIN=$(pg_config --bindir)

# ── Config ────────────────────────────────────────────────────────────────────
PGDATA=/var/lib/postgresql/kham_bench
PGSOCKET=/var/run/postgresql
PGPORT=15433
PGUSER=postgres
PGLOG=/tmp/pg_kham_bench.log

# ── Prepare directories ───────────────────────────────────────────────────────
mkdir -p "$PGDATA" "$PGSOCKET"
chown postgres:postgres "$PGDATA" "$PGSOCKET"
chmod 700 "$PGDATA"
chmod 775 "$PGSOCKET"
touch "$PGLOG"
chown postgres:postgres "$PGLOG"

# ── Initialise cluster ────────────────────────────────────────────────────────
echo "[bench] Running initdb..."
$RUNAS "$PG_BIN/initdb" -D "$PGDATA" --encoding=UTF8 --locale=C

echo "dynamic_shared_memory_type = mmap" >> "$PGDATA/postgresql.conf"

printf 'local all all trust\nhost  all all 127.0.0.1/32 trust\n' \
    >> "$PGDATA/pg_hba.conf"

# ── Start server ──────────────────────────────────────────────────────────────
echo "[bench] Starting PostgreSQL..."
$RUNAS "$PG_BIN/pg_ctl" start -D "$PGDATA" -l "$PGLOG" -t 60 \
    -o "-p $PGPORT -k $PGSOCKET -c listen_addresses='' -c dynamic_shared_memory_type=mmap" \
    || { echo "[bench] ERROR: server failed to start:"; cat "$PGLOG"; exit 1; }

i=0
while [ $i -lt 30 ]; do
    $RUNAS "$PG_BIN/pg_isready" -h "$PGSOCKET" -p "$PGPORT" -U "$PGUSER" -q 2>/dev/null \
        && break
    i=$((i + 1))
    sleep 1
done
if [ $i -eq 30 ]; then
    echo "[bench] ERROR: server not ready after 30s:"; cat "$PGLOG"; exit 1
fi
echo "[bench] PostgreSQL ready"

# ── Create benchmark database ─────────────────────────────────────────────────
$RUNAS "$PG_BIN/createdb" -h "$PGSOCKET" -p "$PGPORT" -U "$PGUSER" kham_bench

# ── Run benchmark ─────────────────────────────────────────────────────────────
echo "[bench] Running benchmark..."
$RUNAS "$PG_BIN/psql" \
    -h "$PGSOCKET" -p "$PGPORT" -U "$PGUSER" -d kham_bench \
    -f /kham/bench/bench.sql

echo "[bench] Done."
