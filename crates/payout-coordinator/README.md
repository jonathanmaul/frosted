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

## Resolution and payout

The worker resolves chain markets once the configured resolve height is final. It independently
reconstructs escrow notes with `glass-indexer` in `COORDINATOR_INDEXER_DB_PATH` (default
`./private/indexer.db`); no raw memo or private note identifier is fetched from the public API.
The coordinator database has an exclusive process lock. Planning and broadcast operations are
serialized, and signed transaction bytes and the encrypted wallet snapshot are committed before
broadcast. Restarting retries those same bytes.

```sh
frosted-operator payout plan MARKET_ID
frosted-operator payout status MARKET_ID
frosted-operator payout sign MARKET_ID --plan-hash PLAN_HASH \
  --share MEMBER_1.share.enc --member-key member-1.key \
  --share MEMBER_2.share.enc --member-key member-2.key
frosted-operator payout follow-up MARKET_ID
```

Each member must type the complete plan hash before their share is decrypted. Approval expires
after `SIGNING_SESSION_TIMEOUT_SECS` (600 by default). Shares and fresh FROST nonces stay in the
operator process. Only signatures travel over localhost to the coordinator.

For batches exceeding `MAX_ACTIONS_PER_TX`, intermediate funding outputs return to escrow.
After each step confirms, run `payout sign` again with the same plan hash and approve the next
step. The published fee includes every funding transaction. Whole WIN/REFUND outputs are never
split or reduced. Expired transactions are rebuilt with a fresh anchor; signing must be approved
again. Follow-up planning excludes coordinator-generated funding notes and only handles notes
received after the previous confirmed cutoff.
