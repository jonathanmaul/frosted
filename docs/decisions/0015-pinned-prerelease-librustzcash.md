# ADR-0015: librustzcash pre-release yang dikunci untuk PCZT Ironwood (amend ADR-0010)

- Status: Accepted
- Tanggal: 2026-10-21
- Bagian spec: §14 (dependensi), §25 (Zcash core), M7, M8
- Bukti: `docs/SPIKES.md` §M0.2, §M0.4

## Context

ADR-0010 mewajibkan crate librustzcash upstream yang dirilis di crates.io. Spike menemukan:
- Dekripsi IVK Ironwood (M0.2) jalan dengan grup **stabil**: `orchard` 0.15.5 + `zcash_primitives` 0.30.1.
- PCZT dengan spend Ironwood (M0.4) hanya terbukti jalan dengan grup **pre-release**: librustzcash git rev `eb38d7a` (`pczt` 0.10.0-pre.2, `zcash_client_backend` 0.25.0-pre.2, `orchard` 0.16). Grup stabil `pczt` 0.9.3 / `zcash_client_backend` 0.24.0 belum mendukung alur ini.

## Decision

- Indexer, verifier, dan scan (`zcash-glass` feature `zcash`) memakai grup **stabil** dari crates.io selama cukup.
- `payout-coordinator` dan `frosted-operator` (PCZT, wallet escrow) memakai grup **pre-release** yang dikunci ke rev git `eb38d7a` lewat `[patch.crates-io]`, satu rev untuk seluruh grup.
- Upgrade rev hanya lewat change baru, dengan regresi M0.4 diulang.
- Begitu rilis stabil mendukung PCZT Ironwood, kedua sisi pindah ke versi stabil dan ADR ini ditutup.

## Consequences

- Dua grup versi librustzcash bisa hidup di workspace yang sama. Tipe dari grup berbeda tidak boleh saling dipertukarkan; batasnya adalah format data (bytes/string), bukan tipe Rust.
- ADR-0010 tetap berlaku sebagai default; ADR ini adalah pengecualian yang tercatat.
