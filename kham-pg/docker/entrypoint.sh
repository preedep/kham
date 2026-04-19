#!/usr/bin/env sh
# entrypoint.sh — initialise PostgreSQL, run pg_regress, report result.
#
# Works on both Alpine (su-exec) and Debian (gosu).
# Paths are resolved via pg_config so this script is distro-agnostic.
# Runs as root; DB operations are delegated to the postgres user.
set -eu

# ── Detect privilege-drop helper ─────────────────────────────────────────────
if command -v su-exec >/dev/null 2>&1; then
    RUNAS="su-exec postgres"
elif command -v gosu >/dev/null 2>&1; then
    RUNAS="gosu postgres"
else
    echo "[entrypoint] ERROR: neither su-exec nor gosu found" >&2
    exit 1
fi

# ── Resolve PG paths via pg_config ───────────────────────────────────────────
PG_BIN=$(pg_config --bindir)
PGXS_MK=$(pg_config --pgxs)                          # .../pgxs/src/makefiles/pgxs.mk
PG_REGRESS=$(dirname "$(dirname "$PGXS_MK")")/test/regress/pg_regress

if [ ! -x "$PG_REGRESS" ]; then
    PG_REGRESS=$(find /usr/lib/postgresql* /usr/local/lib/postgresql* \
                      -name pg_regress -type f 2>/dev/null | head -1)
fi

# ── Config ────────────────────────────────────────────────────────────────────
PGDATA=/var/lib/postgresql/kham_test
PGSOCKET=/var/run/postgresql
PGPORT=15432
PGUSER=postgres
PGLOG=/tmp/pg_kham.log

# ── Prepare directories ───────────────────────────────────────────────────────
mkdir -p "$PGDATA" "$PGSOCKET"
chown postgres:postgres "$PGDATA" "$PGSOCKET"
chmod 700 "$PGDATA"
chmod 775 "$PGSOCKET"
touch "$PGLOG"
chown postgres:postgres "$PGLOG"

# ── Initialise cluster ────────────────────────────────────────────────────────
echo "[entrypoint] Running initdb..."
$RUNAS "$PG_BIN/initdb" -D "$PGDATA" --encoding=UTF8 --locale=C

# mmap is required for Docker containers (shared memory via files, not POSIX shm)
echo "dynamic_shared_memory_type = mmap" >> "$PGDATA/postgresql.conf"

printf 'local all all trust\nhost  all all 127.0.0.1/32 trust\n' \
    >> "$PGDATA/pg_hba.conf"

# ── Start server ──────────────────────────────────────────────────────────────
echo "[entrypoint] Starting PostgreSQL..."
$RUNAS "$PG_BIN/pg_ctl" start -D "$PGDATA" -l "$PGLOG" -t 60 \
    -o "-p $PGPORT -k $PGSOCKET -c listen_addresses='' -c dynamic_shared_memory_type=mmap" \
    || { echo "[entrypoint] ERROR: server failed to start:"; cat "$PGLOG"; exit 1; }

# Wait for socket to be ready
i=0
while [ $i -lt 30 ]; do
    $RUNAS "$PG_BIN/pg_isready" -h "$PGSOCKET" -p "$PGPORT" -U "$PGUSER" -q 2>/dev/null \
        && break
    i=$((i + 1))
    sleep 1
done
if [ $i -eq 30 ]; then
    echo "[entrypoint] ERROR: server not ready after 30s:"; cat "$PGLOG"; exit 1
fi
echo "[entrypoint] PostgreSQL ready"

# ── Create regression database ────────────────────────────────────────────────
$RUNAS "$PG_BIN/createdb" -h "$PGSOCKET" -p "$PGPORT" -U "$PGUSER" regression

# ── Run pg_regress ────────────────────────────────────────────────────────────
echo "[entrypoint] Using pg_regress: $PG_REGRESS"
cd /kham/kham-pg/regress
chown -R postgres:postgres .

# --outputdir=. means pg_regress writes output/ and results/ directly under cwd
# (i.e. regress/output/kham_fts.out and regress/results/kham_fts.out)
$RUNAS "$PG_REGRESS" \
    --inputdir=. \
    --outputdir=. \
    --dbname=regression \
    --host="$PGSOCKET" \
    --port="$PGPORT" \
    --user="$PGUSER" \
    kham_fts \
    kham_thai \
    kham_operators
STATUS=$?

if [ $STATUS -ne 0 ]; then
    echo ""
    echo "=== REGRESSION DIFFS ==="
    cat regression.diffs 2>/dev/null || true
fi

exit $STATUS
