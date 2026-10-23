#!/bin/sh
# Runs zebrad and zainod in one container and exits when either stops, so Railway restarts both.
set -eu
mkdir -p /data/zebra /data/zaino
zebrad -c /etc/zebrad/zebrad.toml start &
ZEBRA=$!
# Zaino needs the validator RPC up; retry until it answers.
for _ in $(seq 1 120); do
  curl -sf -X POST -H 'content-type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"getblockchaininfo","params":[]}' http://127.0.0.1:18232 >/dev/null 2>&1 && break
  sleep 5
done
zainod start --config /etc/zainod/zainod.toml &
ZAINO=$!
trap 'kill $ZEBRA $ZAINO 2>/dev/null || true' TERM INT
while kill -0 $ZEBRA 2>/dev/null && kill -0 $ZAINO 2>/dev/null; do sleep 5; done
kill $ZEBRA $ZAINO 2>/dev/null || true
exit 1
