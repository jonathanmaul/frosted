# ADR-0018: Semua sisi memakai librustzcash pre-release agar bisa membaca transaksi NU7 (amend ADR-0015)

- Status: Accepted
- Tanggal: 2026-10-23
- Bagian spec: §14 (dependensi), §25 (Zcash core), M2 (scan), M12 (testnet)
- Bukti: pengujian pada transaksi testnet nyata, 2026-10-23 (lihat Context)

## Context

ADR-0015 menetapkan indexer, verifier, dan scan (`zcash-glass` feature `zcash`) memakai grup **stabil** (`zcash_primitives` 0.30.1, `orchard` 0.15.5) "selama cukup".

Testnet publik melakukan NU7 di block 4.465.026 (dokumentasi Chainstack, dikonfirmasi dari hash block di endpoint Chainstack). Transaksi sesudahnya berformat v6 dengan consensus branch id `0x77190ad9`.

Pengujian: 23 transaksi shielded NU7 dari testnet (blok 4.532.097 sampai 4.532.176, termasuk transaksi dompet kita sendiri) diurai dengan `ParsedTx::parse`.

| Grup | Hasil |
| --- | --- |
| Stabil (`zcash_primitives` 0.30.1) | 0 dari 23 berhasil: `invalid consensus branch id 0x77190ad9` |
| Pre-release (`zcash_primitives` 0.31.0-pre.2) | 23 dari 23 berhasil |

Artinya indexer tidak bisa membaca satu pun transaksi testnet sekarang. `wallet-helper` sudah memakai grup pre-release (workspace terpisah) dan berhasil membangun serta menyiarkan transaksi NU7.

## Decision

- Workspace utama (`zcash-glass`, indexer, verifier, API, coordinator, WASM) pindah ke versi pre-release dari crates.io yang sama dengan `wallet-helper`: `zcash_primitives` 0.31.0-pre.2, `zcash_keys` 0.17.0-pre.2, `zcash_protocol` 0.11.0-pre.1, `zcash_address` =0.14.0-pre.2, `orchard` 0.16.0, `zcash_note_encryption` 0.5.1.
- NU7 dimodelkan: `ActivationHeights.nu7` dibaca dari `getblockchaininfo`, dan cabang konsensus dipilih dari tinggi aktivasi.
- Versi dikunci lewat `Cargo.lock`. Pindah ke rilis stabil begitu rilis stabil yang mengenal NU7 tersedia, lalu ADR ini ditutup.

## Consequences

- Satu generasi librustzcash di seluruh repo, jadi pembatasan "tipe dari grup berbeda tidak boleh bertukar" di ADR-0015 tidak berlaku lagi di antara workspace utama dan `wallet-helper`.
- Regresi: `cargo test --workspace` 146 lulus, `clippy -D warnings` bersih, build `wasm32` berhasil, dan 17 tes Playwright di regtest tetap lulus.
- Perubahan API yang ikut: `UnifiedAddress::from_receivers` menerima lima argumen, `unified::Address::decode` mengembalikan `(network, revision, address)`, `encode` untuk UFVK dan UIVK mengembalikan `Result`.
- Risiko: pre-release bisa berubah sebelum stabil. Karena itu versinya dikunci di lockfile dan dites terhadap transaksi nyata.
- Belum terbukti: alur lengkap (market, indexer, payout) di testnet. Yang terbukti baru parser dan `wallet-helper` secara terpisah.
