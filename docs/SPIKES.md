# SPIKES.md — Hasil spike M0

Catatan kelayakan teknis Frosted. Setiap bagian menjawab item `[TO VERIFY]` (V-x) di `docs/PROJECT_SPEC_FROSTED.md` §36.

Aturan:
- Semua spike M0.2–M0.5 berjalan di regtest laptop lokal (`scripts/dev-up.sh`); verifikasi node live ditunda ke M12 (ADR-0012).
- Status: `PASS` / `FAIL` / `PARTIAL` (sementara boleh `PENDING (waiting for operator)`, `DEFERRED (to Mx)`, atau `NOT STARTED`).
- Bukti = perintah persis + output atau txid, sehingga bisa diulang.
- Hasil yang bertentangan dengan spec → ADR baru di `docs/decisions/` + usulan perubahan spec (§28).
- Semua spike dikerjakan di regtest laptop lokal; verifikasi node live ditandai "deferred to M12" (ADR-0012).

## Ringkasan go/no-go

| Spike | Status | Keputusan |
| --- | --- | --- |
| M0.1 regtest env | PASS | Regtest Ironwood siap; semua spike berikutnya memakai stack ini |
| M0.2 Ironwood IVK decrypt | PASS | Glass Escrow layak: note Ironwood terdekripsi hanya dengan IVK |
| M0.3 FVK dari FROST | PASS | FROST `ak` + `nk`/`rivk` → FVK → UFVK/UIVK Orchard-only berfungsi |
| M0.4 FROST + PCZT + Zaino | PARTIAL | Spend escrow ditandatangani FROST 2-of-3 di PCZT dan terkonfirmasi; transport `frostd`/`frost-client` tidak kompatibel (FROST 2.x vs 3.0) → perlu ADR, tidak perlu fallback single-key |
| M0.5 valuePools historis | PASS (regtest) | `getblock(h, 1)` memberi `valuePools` per blok; live deferred to M12 |
| M0.6 Zodl memo | PARTIAL: regtest PASS, Zodl DEFERRED (to M6) | Bukan blocker; Zodl testnet hanya build iOS dari source. Dijalankan saat Bet Builder (M6) siap |

### Status V-1..V-14

| V | Jawaban di | Status |
| --- | --- | --- |
| V-1 | M0.2 | Terjawab: kiriman ke receiver Orchard pasca-NU6.3 → bundle Ironwood |
| V-2 | M0.6 | Menunggu operator |
| V-3 | M0.4 | Terjawab: ya, spend Ironwood ditandatangani FROST 2-of-3 |
| V-4 | M0.3 | Terjawab |
| V-5 | M0.3 | Terjawab: tidak ada item UFVK Ironwood terpisah di upstream |
| V-6 | M0.2 | Terjawab: `IncomingViewingKey::diversifier_index` |
| V-7 | M0.5 | Terjawab: ya |
| V-8 | M0.4 | Terjawab |
| V-9 | M0.4 | Terjawab: v3 di `reddsa` 0.5.2; `frostd` masih v2 → ADR |
| V-10 | M0.2 | Terjawab: `Transaction::ironwood_bundle()` |
| V-11 | M0.4 | Terjawab: Zaino 0.10.1 melayani sync Ironwood |
| V-12 | — | Tidak relevan lagi (ADR-0009) |
| V-13 | M0.1 | Terjawab |
| V-14 | M0.6 | Faucet ditemukan, belum terverifikasi berfungsi |

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

- **Status:** PASS
- **Versi crate & tool yang terbukti jalan:** `orchard` 0.15.5, `zcash_primitives` 0.30.1, `zcash_keys` 0.16.1 (feature `orchard`), `zcash_note_encryption` 0.4.2, `zcash_protocol` 0.10.6 (feature `local-consensus`). Grup stabil ini memakai `orchard` 0.15.x (`orchard` 0.16 hanya dipakai grup pre-release). Spike: `spikes/s2-ironwood-ivk-decrypt`.
- **Nama fungsi/API yang benar:**
  - **V-10:** `Transaction::read(&bytes[..], BranchId::Nu6_3)` mem-parse tx v6 (`tx.version()` = `V6`); bundle Ironwood lewat `tx.ironwood_bundle() -> Option<&orchard::Bundle<_, ZatBalance>>` (terpisah dari `tx.orchard_bundle()`).
  - **V-1:** kiriman ke UA Orchard-only pasca-NU6.3 menghasilkan aksi di bundle **Ironwood**, terdekripsi dengan `orchard::note_encryption::IronwoodDomain` (note plaintext versi V3, lead byte `0x03`; `OrchardDomain` hanya menerima V2).
  - Dekripsi: `zcash_note_encryption::try_note_decryption(&IronwoodDomain::for_action(action), &ivk.prepare(), action) -> Option<(Note, Address, [u8; 512])>`.
  - **V-6:** `orchard::keys::IncomingViewingKey::diversifier_index(&Address) -> Option<DiversifierIndex>` (juga `FullViewingKey::diversifier_index(&Diversifier)`); turunan alamat `IncomingViewingKey::address_at(j)`.
- **Bukti:**
  ```
  $ spikes/s2-ironwood-ivk-decrypt/target/release/s2-ironwood-ivk-decrypt address 1
  uregtest1n6ct38cjyvfcxt5u5amkqqt3rvrg0n09laepvzchhp8as9u2687jl5vx8pw647u2m9enhyj4pwlsya4xp8nv07qhl3jhdxjmxqelc2ha
  $ scripts/regtest-wallet.sh dt wallet -w /work/faucet send ... --address <UA di atas> --value 2500000 --memo "FRST1:m0.2-spike"
  baf8efd37ec597e7e2227b38cc5b33e1f36e7edb8347f29d15f42e336196bffa
  $ curl getrawtransaction [txid, 0] > /tmp/m02.hex && s2-ironwood-ivk-decrypt decrypt /tmp/m02.hex
  txid baf8efd3...bffa version V6
  action 1: value=2500000 zat diversifier_index=Some(1) memo="FRST1:m0.2-spike"
  ```
- **Dampak ke desain:** tidak ada perubahan wajib. Catatan untuk §19.4/§25: domain dekripsi untuk bet adalah `IronwoodDomain` (bukan `OrchardDomain`), dan versi `orchard` stabil yang cocok dengan `zcash_primitives` 0.30.1 adalah 0.15.x. M2.7 (`scan`) boleh dimulai.

## M0.3 — `s3-fvk-from-frost`

**Menjawab:** V-4, V-5.

- **Status:** PASS
- **Versi crate & tool yang terbukti jalan:** `reddsa` 0.5.2 (feature `frost`, memakai **`frost-rerandomized` 3.0.0 / `frost-core` 3.0.0**), `orchard` 0.15.5, `zcash_keys` 0.16.1 (features `orchard`, `unstable-frost`), `pasta_curves` 0.5. Spike: `spikes/s3-fvk-from-frost`.
- **Nama fungsi/API yang benar:**
  - **V-4:** `reddsa::frost::redpallas::keys::generate_with_dealer(3, 2, IdentifierList::Default, rng)` sudah menerapkan `EvenY`, sehingga `PublicKeyPackage::verifying_key().serialize()` (32 byte) langsung valid sebagai `ak`. `orchard::keys::FullViewingKey::from_bytes(&[u8; 96])` (`ak ‖ nk ‖ rivk`) publik dan memvalidasi `ak` (`SpendValidatingKey::from_bytes`), `nk`, `rivk`, serta ivk ≠ 0. `nk` = `pallas::Base::random().to_repr()`, `rivk` = `pallas::Scalar::random().to_repr()`.
  - **V-5:** `zcash_keys::keys::UnifiedFullViewingKey::from_orchard_fvk(fvk)` (feature `unstable-frost`), `.to_unified_incoming_viewing_key()`, `.encode(&params)` → `uviewregtest1…` / `uivkregtest1…`; `UnifiedIncomingViewingKey::decode(&params, s)?.orchard()`. **Upstream `zcash_keys` 0.16.1 tidak punya item UFVK khusus Ironwood**: Ironwood memakai item Orchard yang sama (tipe `IronwoodFvk` hanya ada di fork `zakura-client-sqlite`). Dompet `zcash-devtool` (`init-fvk`) menerima UFVK ini dan mengenali saldo Ironwood-nya.
  - Turunan Outcome Address dari UIVK saja: `ivk.address_at(j)` sama dengan `fvk.address_at(j, Scope::External)` (di-assert untuk j = 0..2).
- **Bukti:**
  ```
  $ s3-fvk-from-frost dealer .regtest/escrow-m03.json
  ak   70ae3d069e82758d596ef98540bb56b1ac83ac224c7753273e49020b4200a903
  UIVK uivkregtest19e6ysaaf26dyc3ay3st73rkw6y4wl5npde3l7jnqy7qr9t7y8ag0a5ehgg4ya3hy0esm7stnjh6gwxsg0calkpd8c9ehw6wtq5etvae0773t3p9avl6tehx5jyj6h2p2650sexgszy
  outcome 0 uregtest1tlkfh6...6hzam
  outcome 1 uregtest1gvjrxm...469jk
  $ faucet send 3000000 zat, memo "FRST1:m0.3-escrow" → outcome 1
  dd1f10c29afd4fd06fbf97d331cdbf508029474030755a558623769232ee7ca7
  $ s3-fvk-from-frost decrypt .regtest/escrow-m03.json /tmp/m03.hex     # hanya memakai UIVK
  action 0: value=3000000 zat outcome_index=Some(1) memo="FRST1:m0.3-escrow"
  ```
  (Escrow ini kunci uji regtest; `.regtest/escrow-m03.json` gitignored.)
- **Dampak ke desain:** fallback single-key **tidak** diperlukan. Perlu koreksi §25/SD-4/T-7: `reddsa` 0.5.2 sudah memakai FROST **v3** (`frost-rerandomized` 3.0.0), bukan 2.x; `reddsa` 0.6.x membuang feature `frost` sama sekali. Usulan ADR di laporan spike.

## M0.4 — `s4-frost-pczt-sign`

**Menjawab:** V-3, V-8, V-9, V-11, V-22, V-28.

- **Status:** PARTIAL — kriptografi dan alur PCZT terbukti ujung ke ujung (tx terkonfirmasi); transport `frostd` + `frost-client` tidak dijalankan karena tidak kompatibel versi (lihat V-9).
- **Versi crate & tool yang terbukti jalan:** `pczt` 0.10.0-pre.2, `zcash_primitives` 0.31.0-pre.2, `zcash_protocol` 0.11.0-pre.1 (librustzcash upstream git rev `eb38d7a`, sama dengan image `zcash-devtool`), `orchard` 0.16.0, `reddsa` 0.5.2 + `frost-rerandomized` 3.0.0 untuk FROST. Wallet escrow view-only, sync, `pczt create/prove/send`: `zcash-devtool` `0b3436b` (librustzcash rev `eb38d7a`, `zcash_client_backend` 0.25.0-pre.2, `zcash_client_sqlite` 0.23.0-pre.2) lewat Zaino 0.10.1. Spike signer: `spikes/s4-frost-pczt-sign`.
- **Nama fungsi/API yang benar:**
  - **V-11:** Zaino 0.10.1 melayani sync wallet Ironwood. `zcash-devtool wallet init-fvk --fvk <UFVK>` + `wallet sync` lewat Zaino menghasilkan `Ironwood Spendable: 0.03` untuk escrow FROST. Fallback `z_gettreestate` (**V-22**) tidak diperlukan (Zebra memang menyediakan RPC `z_gettreestate`).
  - **V-8:** sighash: `pczt::roles::signer::Signer::new(pczt)?.shielded_sighash() -> [u8; 32]`. `alpha` per aksi: `pczt::roles::low_level_signer::Signer::new(pczt).sign_ironwood_with(|_, bundle, _| …)` lalu `action.spend().alpha()` (getter publik di `orchard::pczt::Spend`). Sisipkan tanda tangan eksternal: `Signer::apply_ironwood_signature(index, orchard::primitives::redpallas::Signature::from([u8; 64]))`, yang memverifikasi terhadap `rk` aksi. **Dummy spend** sudah ditandatangani oleh IO Finalizer saat `pczt create` (`dummy_sk` lalu dihapus), sehingga signer cukup melewati aksi yang `spend_auth_sig()` sudah terisi. `zcash-devtool pczt update-with-signature` belum mendukung Ironwood (TODO di source), sehingga spike menyisipkan tanda tangan sendiri.
  - **V-3:** FROST RedPallas **bisa** mengotorisasi spend Ironwood: tanda tangan 2-of-3 teragregasi lolos `apply_ironwood_signature` dan tx terkonfirmasi.
  - **V-9:** `reddsa` 0.5.2 = FROST v3. Randomizer eksternal (alpha dari PCZT, SD-4 butir 2) hanya tersedia lewat API **deprecated**: `frost_rerandomized::sign(signing_package, nonces, key_package, Randomizer)` + `RandomizedParams::from_randomizer(group_vk, Randomizer::deserialize(alpha))` + `reddsa::frost::redpallas::rerandomized::aggregate(...)`. Jalur non-deprecated (`sign_with_randomizer_seed`) menurunkan randomizer dari seed + commitments, sehingga tidak cocok dengan alpha yang sudah tertanam di proof. `frost-zcash-demo` (`frostd`/`frost-client`, commit `53df99c`) masih memakai FROST 2.x (`frost-core` 2.2, `frost-rerandomized` 2.0.0-rc.0, `reddsa` git `ed49e9c`), jadi key package v3 tidak bisa dipakai bersama.
  - **V-28:** fee ZIP 317 aktual = 10.000 zat untuk 2 aksi Ironwood (1 spend nyata + 1 dummy) = `5000 × max(2, actions)`.
  - **Biaya signing (D-4):** 1 sesi FROST per spend escrow nyata; aksi dummy tidak butuh sesi. Durasi round1 + round2 + aggregate di proses yang sama (tanpa jaringan): 2–7 ms per sesi. Prove PCZT (Halo 2, 2 aksi) ≈ 2,6 s termasuk start container. Kemampuan multi-pesan per sesi di `frostd`: belum diuji (lihat status).
  - Catatan: `wallet init-fvk` di devtool tidak menerima `--activation-heights` dan memakai default regtest devtool (termasuk **NU7 di tinggi 2**), sehingga tx pertama ditolak dengan `invalid consensus branch id 0x77190ad9`. Diperbaiki dengan menyalin activation heights yang diturunkan dari zebrad ke `keys.toml` wallet. Branch id NU7 di librustzcash (`0x77190ad9`) juga belum sama dengan Zebra 6.4.2 (placeholder `0xfffffffe`).
- **Bukti:**
  ```
  $ scripts/regtest-wallet.sh dt wallet -w /work/escrow init-fvk --name escrow --fvk <UFVK M0.3> --birthday 600 -s 127.0.0.1:8137
  $ ... wallet -w /work/escrow sync / balance          → Ironwood Spendable: 0.03000000 REG
  $ ... pczt -w /work/escrow create --address <recipient Orchard-only UA> --value 1000000 --memo "FRST1:m0.4-payout" --output /work/m04.pczt
  $ ... pczt -w /work/escrow prove /work/m04.pczt --output /work/m04.proven.pczt
  $ spikes/s4-frost-pczt-sign/target/release/s4-frost-pczt-sign sign .regtest/escrow-m03.json .regtest/m04.proven.pczt .regtest/m04.signed.pczt
  shielded sighash 36f293480b4c530568bd479e0bb357e2dba516dfbdad163509d98b164855c3b0
  action 0: FROST 2-of-3 signature applied (2 ms)
  action 1: already signed by the IO Finalizer (dummy spend)
  FROST signing sessions: 1
  $ ... pczt -w /work/escrow send /work/m04.signed.pczt -s 127.0.0.1:8137
  b0c35548168bd9093516adbddf16a5dbe257b30b9e47bd6805534c0b4893f236
  $ getrawtransaction [txid, 1]   → confirmations 3, height 742, version 6
  recipient list-tx                → Amount: 0.01000000 REG
  ```
- **Dampak ke desain:** fallback single-key **tidak** diperlukan. Perlu ADR (usulan di laporan): (1) stack FROST = `reddsa` 0.5.2 + `frost-rerandomized` 3.0.0, randomizer eksternal lewat API deprecated (risiko penghapusan di versi mendatang; alternatif v3-native: jalankan round 1 FROST lebih dulu, turunkan randomizer dari commitments, dan set `alpha` ke PCZT sebelum proving); (2) `frostd`/`frost-client` dari `frost-zcash-demo` tidak bisa dipakai apa adanya → transport signing perlu diputuskan (port ke FROST v3, atau signing in-process di `frosted-operator` untuk demo satu orang, ADR-0008); (3) ADR-0007 tetap berlaku. `zcash_client_backend` yang benar-benar dipakai untuk PCZT Ironwood adalah grup **pre-release** (git rev), bukan 0.24.0 stabil — keputusan versi M7/M8 perlu mengikuti ini.

## M0.5 — `s5-valuepools-at-height`

**Menjawab:** V-7, V-20, V-27 (regtest; live deferred to M12).

- **Status:** PASS (regtest). Node live: **deferred to M12** (M12.0).
- **Versi crate & tool yang terbukti jalan:** `zfnd/zebra:6.4.2`, RPC JSON-RPC langsung (tanpa crate). Spike: perintah RPC (folder `spikes/s5-valuepools-at-height` tidak diperlukan).
- **Nama fungsi/API yang benar:**
  - **V-7:** `getblock(height_as_string, 1)` mengembalikan `valuePools[]` per blok pada tinggi historis, dengan field `id`, `chainValueZat` (nilai pool **setelah** blok itu) dan `valueDeltaZat`, plus `chainSupply`. Fallback snapshot SD-3 tidak diperlukan.
  - **V-20:** id pool = `transparent`, `sprout`, `sapling`, `orchard`, `lockbox`, `ironwood` (`sprout` ada di respons; daftar `POOL_THRESHOLD` §22.3 tidak memuatnya, dan itu tidak masalah). Nama upgrade di `getblockchaininfo.upgrades[].name`: `"NU5"`, `"NU6"`, `"NU6.1"`, `"NU6.2"`, `"NU6.3"`; **NU7 tidak muncul sama sekali** bila tidak dikonfigurasi, sehingga `UPGRADE_ACTIVE("NU7")` harus memperlakukan "tidak terdaftar" sebagai belum aktif.
  - **V-27 (regtest):** `getblockchaininfo.valuePools` memuat `ironwood` dan `upgrades` memuat `activationheight` + `status`. Live: deferred to M12.
- **Bukti:**
  ```
  $ curl getblock ["5", 1]    → ironwood (chainValueZat 0, valueDeltaZat 0)
  $ curl getblock ["690", 1]  → ironwood (119373285000, 0); transparent (167501715000, 156250000)
  $ curl getblock ["742", 1]  → ironwood (119373275000, -10000)   # blok berisi payout FROST M0.4; delta = fee 10.000 zat
  $ curl getblockchaininfo     → upgrades [... ('NU6.2', 1, 'active'), ('NU6.3', 10, 'active')]
  ```
- **Deferred to M12 (V-27 live):** pada endpoint testnet publik atau node lokal (V-31), jalankan `getblockchaininfo` dan `getblock("<h>", 1)` untuk h di sekitar 4.134.000; harapkan `valuePools` memuat `ironwood` dengan `chainValueZat` dan `upgrades` memuat `NU6.3` di 4.134.000.
- **Dampak ke desain:** tidak ada perubahan wajib. Catatan untuk §22.3/F-07: resolusi `UPGRADE_ACTIVE` harus menangani upgrade yang tidak terdaftar di node.

## M0.6 — `s6-zodl-memo`

**Menjawab:** V-2, V-14, V-25, V-26.

- **Status:** DEFERRED (to M6, diputuskan 2026-10-21)
- **Separuh regtest (2026-10-21): PASS.** URI ZIP-321 berisi memo FRST1 asli 237 byte (vector `frst1[0]` dari `docs/test-vectors/memo.json`, sha256 `c9d844a7744865b2…`) dibayar dengan `zcash-devtool wallet pay --payment-uri` ke UA Orchard-only milik `recipient`. Memo yang diterima identik byte per byte. Jadi encoding URI kita (memo base64url tanpa padding, panjang URI 453 karakter) valid, dan memo 237 byte lolos utuh lewat jalur payment request.
  ```
  $ scripts/regtest-wallet.sh zip321-test
  ==> memo 237 bytes, sha256 c9d844a7744865b2
  ==> URI zcash:uregtest1e3jg6kwz0jfljua79cxy33rhvafamfsp2maypv0lldjzz... (453 chars)
  ==> txid e60b6304f5d9502987d58ba2354d7de9399d5a01ce76c5789cb071280086abab
  PASS: received memo is identical (237 bytes)
  ```
  Yang masih ditunda ke M6 hanya perilaku aplikasi Zodl di HP (V-2 untuk Zodl, V-25).
- **Alasan penundaan:** Zodl dari App Store/Play Store hanya mainnet. Build testnet (scheme `zodl-testnet`) hanya ada untuk iOS dan harus dikompilasi dari source dengan Xcode ([sumber, belum diverifikasi di repo resmi](https://tomevault.io/tome/zodl-inc/zodl-ios)). V-2 bukan blocker: fallback §34 (UI "copy memo" atau wallet lain) tetap tersedia. Spike dijalankan ulang di M6 dengan QR dari Bet Builder sungguhan, memakai salah satu opsi: (a) Zodl iOS testnet build, atau (b) Zodl mainnet dari store dengan ZEC mikro milik operator.
- **Versi crate & tool yang terbukti jalan:** — (Zodl, dijalankan user)
- **Nama fungsi/API yang benar:**
  - **V-26:** tinggi aktivasi NU6.3 testnet = **4.134.000** (sumber: Zebra v6.4.2 `zebra-chain/src/parameters/constants.rs`, `activation_heights::testnet::NU6_3`; mainnet 3.428.143). Branch id NU6.3 = `0x37a5165b`.
  - **V-14 (belum terverifikasi berfungsi):** faucet testnet yang tercantum di ZecHub: `testnet.zecaucet.com` (ejaan dari sumber; cek dulu). Alternatif resmi: minta TAZ di Discord Zcash (Zcash testnet guide).
- **Persiapan:** penerima = alamat testnet Orchard-only dari kunci spike M0.2 (diversifier index 7), supaya memo bisa didekripsi dengan dekriptor M0.2 tanpa wallet kedua:
  - UA: `utest10g3wfuycgmt6pts02urpmv3sfx95q4ak39gdu6ygh8ey3sqf9kcluthm67hfmu0ct3w50h9u68hw3hmtwh4urnj9kf037rv60qmrjaqu`
  - Memo FRST1 dummy: 237 byte, SHA-256 `26f417576de1331e8af229b2ac279ed17d1e112ad86162184faa80a7bbe34375` (teks lengkap di `.regtest/m06-memo.txt`).
  - URI ZIP-321 (memo = base64url tanpa padding dari teks memo):
    ```
    zcash:utest10g3wfuycgmt6pts02urpmv3sfx95q4ak39gdu6ygh8ey3sqf9kcluthm67hfmu0ct3w50h9u68hw3hmtwh4urnj9kf037rv60qmrjaqu?amount=0.001&memo=RlJTVDE6QVFEYzdsenVZZF94alpMZG0tVFJwV1RYQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQlJYaThFU3hiSVZtQ3hKSGo1cFhvdXVZelRDbVpxNkQwd2lBRHh4WUxfS1ViQ0pHQXBtYVEwZnpnYzhzNW1SWkhzM2dzRi1nNDhWNXNZbUdKUkFjSFZMcDVYLTFFM1FJRjNoR3pGb2RDeFVMSVRoSWItTkhweWVFUEt3ZE1rS2VkUUg0ZHE5QWFmajd5alpoQ1M2dzZHRnRON0ZFQkl2eE43UDI4
    ```
- **Instruksi untuk operator (testnet):**
  1. Pasang Zodl dan buat/aktifkan wallet **testnet** (kalau Zodl tidak punya mode testnet, berhenti dan laporkan; opsi berikutnya mainnet mikro dengan nominal ≤ 0,001 ZEC, hanya jika kamu setuju).
  2. Ambil TAZ dari faucet (`testnet.zecaucet.com`) atau minta di Discord Zcash; tunggu sampai saldo shielded tersedia.
  3. Buat QR dari URI di atas (atau tempel URI langsung jika Zodl mendukung paste URI), lalu scan/buka di Zodl.
  4. Di layar konfirmasi, **sebelum kirim**: catat apakah memo terisi otomatis, dan **coba ubah jumlah** (0.001 → 0.002). Catat apakah Zodl mengizinkannya (V-25). Kembalikan ke 0.001.
  5. Kirim dari saldo **shielded**. Catat txid.
  6. Laporkan: txid, apakah memo terisi otomatis, apakah jumlah bisa diedit, versi Zodl, dan screenshot layar konfirmasi bila ada.
- **Setelah laporan (tugas agen, 6.4):** ambil raw tx dari explorer testnet publik (`testnet.zcashexplorer.app`, baca saja) dan jalankan `s2-ironwood-ivk-decrypt decrypt`; bandingkan memo dengan `.regtest/m06-memo.txt` byte per byte (V-2, A-3).
- **Bukti:** —
- **Dampak ke desain:** —

---

## Appendix M4 — API dan temuan dari change `add-glass-indexer`

Diverifikasi dari source crate di registry dan dengan tx regtest (2026-10-21).

- **Branch id per tinggi:** `zcash_protocol::consensus::BranchId::for_height(&params, BlockHeight)` (zcash_protocol 0.10.6, `consensus.rs:811`). Untuk regtest, `params` = `zcash_protocol::local_consensus::LocalNetwork { overwinter, sapling, blossom, heartwood, canopy, nu5, nu6, nu6_1, nu6_2, nu6_3 }` yang diisi dari `getblockchaininfo.upgrades` (ADR-0016). Field `nu7` hanya ada dengan `cfg(zcash_unstable = "nu7")`, jadi NU7 tidak dimodelkan. Dibungkus di `zcash_glass::scan::ScanNetwork`; test: tinggi 5 → `Nu6_2`, tinggi 10 → `Nu6_3`.
- **Parse dan dekripsi:** `Transaction::read(bytes, branch_id)`, `tx.txid()`, `tx.ironwood_bundle()`, `try_note_decryption(&IronwoodDomain::for_action(a), &ivk.prepare(), a)`, `IncomingViewingKey::diversifier_index(&Address)` (sama dengan M0.2). UIVK: `UnifiedIncomingViewingKey::decode(&params, s)?.orchard()`.
- **Z-only Guard:** `tx.transparent_bundle().is_some_and(|b| !b.vin.is_empty())` (`zcash_transparent` 0.10.0, `Bundle::vin`). Terbukti dengan tx shield M0.1 `449ddd79…` (true) dan tx escrow M0.3 `dd1f10c2…` (false).
- **RPC Zebra 6.4.2:** `getbestblockheightandhash` mengembalikan `hash` sebagai **array byte**, bukan hex; follower memakai `getblockcount` + `getblockhash(h)` (hex). `getblock("<h>", 1)` memberi `hash`, `height`, `previousblockhash`, dan `tx` (txid berurutan). `getblockchaininfo.chain` bernilai `"test"` untuk testnet **dan** regtest, jadi jaringan diambil dari `FROSTED_NETWORK`, tidak ditebak.
- **Reorg:** `invalidateblock(hash)` tersedia di regtest. Zebra **tidak** mengembalikan transaksi dari blok yang di-invalidate ke mempool; wallet pengirim (devtool) menyiarkan ulang tx itu belakangan. Indexer menghapus bet dari blok yang tergantikan, sesuai spec.
- **Konflik versi crate (penting untuk semua change berikutnya):** `zcash_primitives` 0.30.1 mengunci `crypto-common = "=0.2.0-rc.1"`. Akibatnya generasi RustCrypto yang memakai `crypto-common` 0.2 stabil (`chacha20poly1305` 0.11, `hkdf` 0.13, `sha2` 0.11, `digest` 0.11, dan `sqlx` 0.9 lewat `sqlx-mysql`) **tidak bisa** ada di graf yang sama. Solusi: `zcash-glass` memakai `chacha20poly1305` 0.10.1, `hkdf` 0.12, `sha2` 0.10 (algoritma identik; semua test vector tetap lulus) dan `glass-indexer` memakai `sqlx` 0.8.6. Rust 1.94 tetap dipakai (tidak lagi diwajibkan `sqlx`, tapi di atas MSRV semua crate).
- **Performa (NFR-F1, regtest):** 20 Market, 17 blok, 4 note: 0,29 s total, **≈ 17 ms/blok**. Blok regtest kecil; ukur ulang pada testnet di M12.
- **Bukti integrasi:** `scripts/it-indexer.sh` (15 pemeriksaan PASS: ANNOUNCING, announcement sah/palsu, COUNTED setelah 3 konfirmasi, IGNORED_DUST, UNECONOMIC_DUST, UNKNOWN_OUTCOME, INVALID_MEMO, Persona sekali, event, restart, reorg, LATE, VOID karena timeout).


## Lampiran M3 — Browser Bet Builder / Unified Address

Source `zcash_address` **0.13.0** diperiksa langsung dari registry Cargo:
`src/kind/unified.rs:384–455` menyediakan `unified::Encoding::decode(&str)` →
`(zcash_protocol::consensus::NetworkType, unified::Address)`, lalu
`unified::Container::items()` / `items_as_parsed()` membuka receiver. Enum
`src/kind/unified/address.rs:12–16` memuat `Receiver::Orchard([u8; 43])`.
Fallback `zcash_keys::address::UnifiedAddress::orchard()` tidak diperlukan.
Parser menolak network yang berbeda; mainnet, testnet, dan regtest memiliki
HRP berbeda. UA rusak atau tanpa Orchard memakai `PAYOUT_ADDRESS_NO_ORCHARD`.

Feature kecil `payout-address` mengaktifkan parser; feature `zcash` juga
mengaktifkannya agar API native tetap sesuai desain. Binding WASM memakai
`payout-address` saja untuk menghindari dependensi scanning/transaction yang
bukan kebutuhan browser. Default core tetap tidak mengaktifkan crate Zcash.

`zcash-glass-wasm` memakai wasm-bindgen **0.2.129**, serde-wasm-bindgen **0.6.5**,
dan getrandom **0.3.4**; fitur `wasm_js` hanya di target wasm32. Core menerima
randomness dari pemanggil. Fungsi JS tidak melakukan fetch atau menyimpan data;
`env_esk`, `bet_salt`, dan `k_root` hanya kembali ke pemanggil.

Verifikasi native: 3 test membandingkan payload lengkap dengan
`docs/test-vectors/envelope.json`, mengurai URI memakai zip321 **0.9.0**,
memverifikasi tag/persona, commitment, proof, dan kasus input ditolak.
URI menggunakan pembagian/modulo integer zatoshi, tanpa floating point.

Paket: dari `web`, jalankan `pnpm build:wasm` (membutuhkan `wasm-pack` **0.15.0**).
Output `web/lib/wasm/pkg` dan salinan runtime `web/public/wasm` diabaikan Git.
Loader `/wasm/zcash_glass_wasm.js` dan WASM relatifnya memakai origin aplikasi.
Halaman `/wasm-test` menerima JSON input uji dan hanya menampilkan status serta
fingerprint; memo mentah dan material receipt tidak dimasukkan ke DOM.
Fixture CLI `cargo run -p zcash-glass-wasm --example build_bet -- INPUT_JSON`
menggunakan randomness tetap **hanya untuk pengujian**, bukan untuk bet nyata.

Ukuran bundle M3: `pnpm build:wasm` menghasilkan **267180 byte** WASM dan
**126086 byte gzip** (wasm-opt + profil `opt-level = "z"`), di bawah 1 MB gzip.
Build native, wasm32, lint web, Next production build, dan clippy binding lulus.

`wasm-pack test --headless --chrome crates/zcash-glass-wasm` lulus **4/4**
di Chrome: shared native vector + URI zip321, validasi input, Persona, dan
semua ekspor JS (WebCrypto, parse UA, receiver commitment, Envelope Proof).
`cargo test -p zcash-glass --features payout-address address::tests` lulus;
`cargo tree -p zcash-glass --edges normal --no-default-features` tidak memuat
`orchard`, `zcash_*` dependency, atau `zip321`. CI memiliki job wasm32 +
headless Chrome tersendiri. Web test suite lulus **22/22**, termasuk isolasi
API ticker pada harness `/wasm-test`.

M3.4 browser `/wasm-test` pada origin `http://localhost:3016`: PASS,
237 memo bytes, 0 runtime exception, seluruh request memakai origin sendiri,
termasuk modul JS dan binary `/wasm/`. Header ticker tidak polling API pada
halaman harness; polling kembali aktif saat navigasi ke halaman biasa.

M3 end-to-end: `scripts/it-wasm.sh` menghasilkan URI dari binding Rust,
membayarnya via `zcash-devtool wallet pay` di regtest, dan membandingkan memo
yang diterima byte per byte. PASS, **237 byte**, SHA-256
`9519d825aa4cab883321db1ce9884ed2c6bfba7fe28d29a539777cca95c40c3c`.
Txid pengujian lokal: `7463fb5790f6d777af06c41a23c571c9bf698eb9dd8faa3f676d83a0c44955b7`.
