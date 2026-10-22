# Private market coordinator

The coordinator binds loopback only. The public API receives signed outgoing
pushes. The wallet helper runs as a child process over stdin/stdout; its SQLite
database exists in memory. The coordinator stores only authenticated encrypted
wallet snapshots, UFVK, per-market key material, envelope key, and hot seed.

Build the root binaries and the separate helper (see `wallet-helper/README.md`):

```sh
cargo build -p frosted-operator -p payout-coordinator
cargo build --locked --manifest-path wallet-helper/Cargo.toml
target/debug/frosted-operator secrets init --dir ./private
```

The initialization command creates a new directory, refuses overwrite, prints
the public `coordinator_pubkey` for `COORDINATOR_PUBKEY` on the API host, and
creates three distinct member encryption keys. Back up those keys separately.
Keep the master key out of database backups. `ENVELOPE_SK_FILE` and
`HOT_WALLET_SEED_FILE` contain encrypted blobs, not raw seeds. Existing plaintext
seed files are rejected.

Configure the documented environment variables (§27.4) and:

```sh
export WALLET_HELPER_BIN="$PWD/wallet-helper/target/debug/frosted-wallet-helper"
export LIGHTWALLETD_GRPC_URL=http://127.0.0.1:9067
export TREASURY_ADDRESS='YOUR_ORCHARD_ONLY_UA'
target/debug/payout-coordinator
```

In another terminal with the same `COORDINATOR_BIND`:

```sh
target/debug/frosted-operator committee init --share-dir ./private/shares \
  --member-key ./private/member-1.key --member-key ./private/member-2.key \
  --member-key ./private/member-3.key
target/debug/frosted-operator hot-wallet init
```

Fund the returned hot address with a shielded regtest payment, mine confirmations,
then create a market with the exact `CreateMarket` JSON in §22.3:

```sh
target/debug/frosted-operator market create --file CreateMarket.json
target/debug/frosted-operator market status MARKET_ID
```

Committee creation is a **trusted dealer 2-of-3**, not distributed DKG. The
coordinator receives only `PublicKeyPackage`; the three encrypted shares remain
outside its database. Market definitions publish the UIVK and outcome addresses.
Creation persists before pushing, so API outages leave a durable pending market.
The worker retries with bounded exponential backoff, signs each attempt with a
fresh timestamp and the same exact body, and synchronizes escrow wallets every
`ESCROW_SYNC_INTERVAL_SECS` (default 30). Activation heights come from zebrad.
Announcements are prepared once: updated hot-wallet snapshot and pending raw
transaction are committed together before broadcasting. Retries broadcast the
same transaction bytes. The public indexer decides `OPEN` and timeout `VOID`.

Remote ingest requires HTTPS; plaintext HTTP is permitted only for loopback
regtest. Redirects are disabled to avoid forwarding signed requests elsewhere.
No wallet requests, snapshots, UFVKs, seeds, or input-dependent helper errors are
logged. M8 resolution, payout planning and signing are outside this change.
