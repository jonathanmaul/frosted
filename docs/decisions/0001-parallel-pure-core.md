# ADR-0001: Modul murni `zcash-glass` boleh dikerjakan paralel dengan M0

- Status: Accepted
- Tanggal: 2026-10-21
- Bagian spec: §28 (aturan umum), M2

## Context

§28 versi 0.1 melarang memulai M2+ sebelum hasil M0 tercatat di `docs/SPIKES.md`. Padahal sebagian besar M2 adalah kode murni tanpa jaringan (params, memo, envelope, thaw, odds, payout, hash) yang tidak bergantung pada hasil spike mana pun. Menunggu M0 selesai memperlambat jalur kritis tanpa mengurangi risiko.

## Decision

- M2.1–M2.6 dan M2.8 boleh dikerjakan paralel dengan M0.
- M2.7 (`scan`) menunggu M0.2, karena bergantung pada API dekripsi Ironwood yang belum terbukti.
- M3+ tetap menunggu hasil M0 tercatat.

## Consequences

- Format memo, Thaw Tag v2, dan hash kanonik (ADR-0002, ADR-0005) harus final sebelum test vector M2 dibuat, karena test vector membekukan format.
- Jika spike memaksa perubahan format (misalnya ukuran receiver), test vector M2 ikut diperbarui lewat ADR baru.
