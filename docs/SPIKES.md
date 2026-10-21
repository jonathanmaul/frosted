# SPIKES.md — Hasil spike M0

Catatan kelayakan teknis Frosted. Setiap bagian menjawab item `[TO VERIFY]` (V-x) di `docs/PROJECT_SPEC_FROSTED.md` §36.

Aturan:
- Status: `PASS` / `FAIL` / `PARTIAL` (sementara boleh `PENDING (waiting for operator)` atau `NOT STARTED`).
- Bukti = perintah persis + output atau txid, sehingga bisa diulang.
- Hasil yang bertentangan dengan spec → ADR baru di `docs/decisions/` + usulan perubahan spec (§28).
- Semua spike dikerjakan di regtest laptop lokal; verifikasi node live ditandai "deferred to M12" (ADR-0012).

## Ringkasan go/no-go

| Spike | Status | Keputusan |
| --- | --- | --- |
| M0.1 regtest env | PASS | Regtest Ironwood siap; semua spike berikutnya memakai stack ini |
| M0.2 Ironwood IVK decrypt | NOT STARTED | — |
| M0.3 FVK dari FROST | NOT STARTED | — |
| M0.4 FROST + PCZT + Zaino | NOT STARTED | — |
| M0.5 valuePools historis | NOT STARTED | — |
| M0.6 Zodl memo | NOT STARTED | — |

---

## M0.1 — `s1-regtest-env` (change `setup-dev-environment`)

**Menjawab:** V-13, V-21, V-27 (regtest), V-18 (versi).

- **Status:** PASS
- **Versi crate & tool yang terbukti jalan:** `zfnd/zebra:6.4.2` (arm64), Zaino `0.10.1` (commit `3244a74`, dibangun dari `infra/zaino/Dockerfile`), `zcash-devtool` commit `0b3436b` dengan feature `regtest_support` (dibangun dari `infra/zcash-devtool/Dockerfile`). Versi crate: lihat "Versi terkunci".
- **Nama fungsi/API yang benar:**
  - **V-13 (config regtest Zebra):** activation height di `[network.testnet_parameters.activation_heights]` dengan kunci serde `"NU5"`, `"NU6"`, `"NU6.1"`, `"NU6.2"`, `"NU6.3"`, `"NU7"` (sumber: `zebra-chain/src/parameters/network/testnet.rs`, `ConfiguredActivationHeights`, v6.4.2). Di Regtest, NU5 dan setelahnya **tidak aktif kecuali dikonfigurasi** (`for_regtest` hanya mengisi Overwinter–Canopy). Mining: RPC `generate(n)` (khusus Regtest); `generatetoaddress` belum diimplementasikan; penerima coinbase = `mining.miner_address` (override env `ZEBRA_MINING__MINER_ADDRESS`). Internal miner butuh compile feature `internal-miner`, tidak dipakai. Port RPC regtest: `18232` (dikonfigurasi). Port gRPC Zaino: `8137` (dipublikasikan ke host sebagai `127.0.0.1:9067`).
  - **V-21 (`zcash-devtool` + Ironwood):** didukung. Build dengan `--features regtest_support`; `wallet init -n regtest --activation-heights <toml>` (kunci `overwinter … nu6_3`, `nu7`, nilai tinggi atau `"never"`), `-s 127.0.0.1:<port>` memakai gRPC plaintext hanya untuk host localhost. Saldo dipecah per pool termasuk `Ironwood Spendable`. Memo baru terlihat di penerima setelah `wallet enhance`.
  - **V-27 (regtest):** `getblockchaininfo.valuePools` memuat id `transparent`, `sprout`, `sapling`, `orchard`, `lockbox`, `ironwood` dengan `chainValueZat`; `upgrades` memuat entri `name: "NU6.3"` dengan `activationheight` dan `status`. Node live: deferred to M12.
  - **Zaino 0.10.x:** activation height regtest diambil dari `getblockchaininfo` validator (tidak dikonfigurasi di Zaino). Menolak alamat validator non-localhost dan listen gRPC non-private tanpa TLS; untuk regtest dibangun dengan feature `no_tls_use_unencrypted_traffic` dan berbagi network namespace dengan zebrad.
- **Bukti:**
  ```
  $ scripts/dev-up.sh
  tip=13 NU6.3 activation=10 status=active
  valuePools: {'transparent': 8125000000, 'sprout': 0, 'sapling': 0, 'orchard': 0, 'lockbox': 0, 'ironwood': 0}
  ==> regtest stack ready (zebrad http://127.0.0.1:18232, Zaino gRPC 127.0.0.1:9067)

  $ scripts/regtest-wallet.sh setup        # coinbase → faucet, jatuh tempo 100 blok, shield
  Ironwood Spendable: 1193.73325000 REG    # shield tx 449ddd79fd2718fd1a7c88d4f450a0952e7a628c434ec052faf43953a3523b2c

  $ scripts/regtest-wallet.sh memo-test    # 0.01 ZEC + memo ke UA Orchard-only milik `recipient`
  Pool: Ironwood   Memo (text): FRST1:test   Fee: 0.00010000 REG
  txid 080be9fd1b138a434ca7f7a3d5183a97cfc0ec205f4d5a68a9226d547d393a44 at tip 675
  full tx fetched: 9166 bytes              # getrawtransaction(txid, 0)
  Memo: Memo::Text("FRST1:test")           # di wallet penerima setelah `enhance`
  ```
  `dev-up.sh` dijalankan ulang saat stack hidup: tip tetap 675 (idempoten, state tidak di-reset).
- **Dampak ke desain:**
  - Kiriman ke UA Orchard-only pasca-NU6.3 menghasilkan output di pool **Ironwood** (indikasi kuat untuk V-1; pembuktian dekripsi IVK tetap di M0.2).
  - Fee 2-action = 10.000 zat, konsisten dengan ZIP 317 `5000 × max(2, actions)` (indikasi untuk V-28; diukur di M0.4).
  - `orchard` 0.16, `reddsa` 0.6.1, dan `frost-rerandomized` 3.0.0 sudah stabil, berbeda dari §25. Perlu dicek di M0.4 apakah varian randomizer-dari-PCZT (SD-4 butir 2, T-7) masih tersedia di FROST v3; jika berubah → ADR.
  - Toolchain Rust 1.94.0 (bukan MSRV librustzcash 1.88) karena `sqlx` 0.9. TypeScript dikunci 5.9.3 (bukan 7.x) karena tooling Next.js masih memakai JS API TypeScript.
  - Dockerfile Zaino upstream gagal di-build (pin versi apt yang sudah hilang); `infra/zaino/Dockerfile` membuang pin tersebut. Image regtest Zaino tanpa TLS tidak boleh dipakai di testnet/mainnet.
  - `zcash-devtool` di host gagal di-build karena `protoc` Homebrew rusak, sehingga wallet uji dijalankan di Docker (tidak menyentuh paket sistem).
  - Perubahan spec: tidak ada yang wajib. `.env.example` §27.4 tetap memakai `LIGHTWALLETD_GRPC_URL=http://127.0.0.1:9067`, sesuai port yang dipublikasikan.

### Versi terkunci (V-18), dicek 2026-10-21

Diambil dari crates.io API dan `npm view`. Rilis stabil terbaru dipakai kecuali disebut lain.

**Toolchain:** Rust **1.94.0** (`rust-toolchain.toml`). MSRV kelompok librustzcash = 1.88 (`orchard` 0.16.0, `zcash_client_backend` 0.24.0, `zcash_primitives` 0.30.1, `pczt` 0.9.3, `reddsa` 0.6.1, `zcash_keys` 0.16.1), tetapi `sqlx` 0.9.0 butuh 1.94.0. Toolchain dikunci ke MSRV tertinggi dari semua dependensi yang direncanakan.

| Kelompok | Crate / paket | Versi stabil terbaru | Catatan vs §25 |
| --- | --- | --- | --- |
| Zcash core | `orchard` | 0.16.0 | §25 menyebut 0.15.x; cek apakah `IronwoodDomain` masih ada (M0.2) |
| | `zcash_primitives` | 0.30.1 (pre: 0.31.0-pre.2) | §25: 0.29.x |
| | `zcash_protocol` | 0.10.6 | sesuai |
| | `zcash_keys` | 0.16.1 | sesuai |
| | `zcash_address` | 0.13.0 | sesuai |
| | `zcash_note_encryption` | 0.5.1 | — |
| | `zcash_client_backend` | 0.24.0 (pre: 0.25.0-pre.2) | §25: ≥ 0.24.0-rc.1, kini stabil |
| | `zcash_client_sqlite` | 0.22.0 | kini stabil |
| | `pczt` | 0.9.3 | sesuai |
| | `zip321` | 0.9.0 | sesuai |
| FROST | `reddsa` | 0.6.1 | §25: 0.5.2. Relevan untuk V-9 (M0.4) |
| | `frost-rerandomized` | 3.0.0 | §25: 2.2.x. FROST v3 stabil; relevan untuk V-9 dan SD-4 butir 2 (M0.4) |
| Kripto app | `x25519-dalek` | 3.0.0 | §25: 2.x |
| | `chacha20poly1305` | 0.11.0 | §25: 0.10.x |
| | `hkdf` / `sha2` | 0.13.0 / 0.11.1 | — |
| | `blake2b_simd` | 1.0.5 | sesuai |
| | `ed25519-dalek` | 3.0.0 | §25: 2.x |
| Backend | `tokio` / `axum` / `sqlx` | 1.54.0 / 0.8.9 / 0.9.0 | `sqlx` 0.9 menaikkan MSRV ke 1.94 |
| | `serde` / `serde_json` / `tracing` | 1.0.229 / 1.0.154 / 0.1.44 | — |
| WASM | `wasm-bindgen` | 0.2.129 | — |
| Lain | `base64` / `rand_core` / `thiserror` / `proptest` | 0.23.1 / 0.10.1 / 2.0.21 / 1.12.0 | — |
| Node | `zebrad` | 6.4.2 (pre: 7.0.0-rc.0, 2026-10-02) | §25: ≥ 6.2.x |
| Indexer | Zaino | 0.10.1 (2026-09-29) | — |
| Wallet uji | `zcash-devtool` | git `main` (commit terakhir 2026-10-17) | tidak dirilis ke crates.io |
| Web | `next` / `react` / `typescript` | 16.4.2 / 19.3.1 / 7.0.2 | §25: Next.js ≥ 15 |
| | `tailwindcss` / `@tailwindcss/postcss` | 4.3.3 / 4.3.3 | — |
| | `eslint` / `eslint-config-next` / `vitest` | 10.13.0 / 16.4.2 / 5.0.5 | — |
| | `idb` / `qrcode` | 8.0.4 / 1.5.4 | — |

Perintah yang dipakai: `curl https://crates.io/api/v1/crates/<crate>` (field `max_stable_version`, `rust_version`), `npm view <pkg> version`, GitHub releases API untuk Zebra dan Zaino.

---

## M0.2 — `s2-ironwood-ivk-decrypt`

**Menjawab:** V-1, V-6, V-10.

- **Status:** NOT STARTED
- **Versi crate & tool yang terbukti jalan:**
- **Nama fungsi/API yang benar:**
- **Bukti:**
- **Dampak ke desain:**

## M0.3 — `s3-fvk-from-frost`

**Menjawab:** V-4, V-5.

- **Status:** NOT STARTED
- **Versi crate & tool yang terbukti jalan:**
- **Nama fungsi/API yang benar:**
- **Bukti:**
- **Dampak ke desain:**

## M0.4 — `s4-frost-pczt-sign`

**Menjawab:** V-3, V-8, V-9, V-11, V-22, V-28.

- **Status:** NOT STARTED
- **Versi crate & tool yang terbukti jalan:**
- **Nama fungsi/API yang benar:**
- **Bukti:**
- **Dampak ke desain:**

## M0.5 — `s5-valuepools-at-height`

**Menjawab:** V-7, V-20, V-27 (regtest; live deferred to M12).

- **Status:** NOT STARTED
- **Versi crate & tool yang terbukti jalan:**
- **Nama fungsi/API yang benar:**
- **Bukti:**
- **Dampak ke desain:**

## M0.6 — `s6-zodl-memo`

**Menjawab:** V-2, V-14, V-25, V-26.

- **Status:** NOT STARTED
- **Versi crate & tool yang terbukti jalan:**
- **Nama fungsi/API yang benar:**
- **Bukti:**
- **Dampak ke desain:**
