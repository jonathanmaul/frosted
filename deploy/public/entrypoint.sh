#!/bin/sh
# Runs the indexer and the API against the same database; exits if either stops so Railway restarts both.
set -eu
mkdir -p "$(dirname "$PUBLIC_DB_PATH")"
# The first run creates the database and applies migrations before the API opens it read-only.
[ -f "$PUBLIC_DB_PATH" ] || glass-indexer once >/dev/null
glass-indexer run &
INDEXER=$!
# Railway injects PORT; the API listens on it when API_BIND is not set explicitly.
[ -z "${PORT:-}" ] || export API_BIND="0.0.0.0:$PORT"
frosted-api &
API=$!
trap 'kill $INDEXER $API 2>/dev/null || true' TERM INT
while kill -0 $INDEXER 2>/dev/null && kill -0 $API 2>/dev/null; do sleep 5; done
kill $INDEXER $API 2>/dev/null || true
exit 1
