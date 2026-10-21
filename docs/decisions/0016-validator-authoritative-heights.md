# ADR-0016: Activation height selalu diturunkan dari validator

- Status: Accepted
- Tanggal: 2026-10-21
- Bagian spec: §19.4, §22.3 (`UPGRADE_ACTIVE`), §27, M0.1, M7.5
- Bukti: `docs/SPIKES.md` §M0.1, §M0.4, §M0.5

## Context

- Wallet view-only `zcash-devtool wallet init-fvk` memakai default regtest (NU7 di tinggi 2), sehingga transaksi pertama ditolak zebrad karena branch id tidak cocok.
- Branch id NU7 di librustzcash pre-release belum sama dengan Zebra 6.4.2.
- Zaino 0.10 sudah menetapkan aturan: activation height validator adalah sumber kebenaran.
- `getblockchaininfo` tidak mencantumkan NU7 sama sekali bila tidak dikonfigurasi.

## Decision

- Semua komponen dan skrip (wallet uji, wallet escrow coordinator, indexer, verifier) **menurunkan activation height dari `getblockchaininfo` validator**, tidak memakai default bawaan library atau file statis.
- Di regtest, NU7 dicatat `"never"`.
- Resolusi `UPGRADE_ACTIVE`: upgrade yang **tidak terdaftar** di `upgrades` diperlakukan sebagai belum aktif (outcome NO), dan bukti resolusi mencatat daftar upgrade yang dilaporkan node.

## Consequences

- `scripts/regtest-wallet.sh` sudah mengikuti aturan ini; M7.5 (wallet escrow) wajib memakai mekanisme yang sama.
- Verifier yang memakai node versi lama bisa melihat NU7 "tidak terdaftar"; ini dicatat sebagai risiko determinisme di §22.3.
