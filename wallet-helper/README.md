# Frosted wallet helper

Isolated Rust workspace for the wallet stack pinned to librustzcash
`eb38d7a373661179dbc083a2fd58b3a6c9b25818`, matching zcash-devtool `0b3436b`.
The root workspace uses the stable scanner stack. Only JSON bytes/strings pass
through stdin/stdout; no librustzcash types cross the boundary.

Build: `cargo build --locked --manifest-path wallet-helper/Cargo.toml`.
Binary: `wallet-helper/target/debug/frosted-wallet-helper`.
On this development Mac, Homebrew protoc has a missing dylib. Upstream supports
using checked-in protobuf bindings when protoc is absent. The verified command
is `env PATH="$HOME/.cargo/bin:/usr/bin:/bin" cargo build --locked --manifest-path wallet-helper/Cargo.toml`.
No dependency source patches or system package changes are required.

One JSON request per process, stdin only. Common fields: `op`, `network`
(`mainnet`, `testnet`, `regtest`), `rpc_url`, `zaino_url`, `birthday_height`.
Regtest activation heights come from zebrad `getblockchaininfo`; no library defaults.
Optional fields are operation-specific:

- `import`: `ufvk`. Imports exactly one `AccountPurpose::ViewOnly` account.
- `hot_init`: `seed_hex`. Creates one spending account and returns a funding address.
- `sync`: `snapshot_b64`. Syncs compact blocks through Zaino.
- `announce`: `snapshot_b64`, `seed_hex`, `address`, `amount_zat`, `memo`.
  Syncs, constructs/proves the payment, and returns `txid` and `tx_hex`.
  **Does not broadcast.**
- `broadcast`: `tx_hex`. Broadcasts the identical prepared bytes and returns `txid`.

Responses include `snapshot_b64` (standard base64), `balance_zat`, nullable
`address`, nullable `txid`, `account_uuid`, and nullable `tx_hex`. Broadcast
response fields other than `txid` are placeholders and must not replace wallet
state. Import/hot_init return a funding address; sync/announce do not rotate it.

The coordinator must encrypt/persist the prepared snapshot and pending `tx_hex`
in one transaction before broadcast. Retries submit the saved identical bytes,
never prepare another payment blindly. A failed helper process leaves the
previous encrypted snapshot unchanged.

SQLite data and compact block cache exist only in memory. `temp_store=MEMORY`
and `journal_mode=MEMORY` prevent SQLite temporary-file spill. A snapshot contains
a plaintext UFVK and all wallet metadata; the coordinator must encrypt it at
rest and must never log requests/responses. There are no helper-side files or
input-dependent error messages. Hot seeds are never embedded in SQLite.
