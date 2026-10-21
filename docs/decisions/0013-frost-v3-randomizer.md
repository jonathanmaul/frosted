# ADR-0013: FROST v3 (`frost-rerandomized` 3.0.0) dengan randomizer dari PCZT

- Status: Accepted
- Tanggal: 2026-10-21
- Bagian spec: §5 butir 4, §25 (FROST), SD-4 `[TO VERIFY]` butir 2, T-7, §36 V-3/V-9
- Bukti: `docs/SPIKES.md` §M0.4

## Context

Spec mengasumsikan `reddsa` 0.5.x + `frost-rerandomized` 2.x dengan randomizer yang diambil dari PCZT. M0.4 menemukan:
- `reddsa` 0.5.2 sudah memakai `frost-rerandomized` **3.0.0** (FROST v3). `reddsa` 0.6.x membuang feature `frost`.
- Di v3, randomizer idealnya diturunkan dari commitment round 1 semua signer. Memakai randomizer yang sudah ditentukan PCZT (`alpha`) hanya bisa lewat API deprecated: `frost_rerandomized::sign(.., Randomizer)` + `RandomizedParams::from_randomizer`.
- Dengan API itu, spend escrow 2-of-3 di dalam PCZT terbukti terkonfirmasi di regtest.

## Decision

- MVP memakai `reddsa` **0.5.2** (feature `frost`) + `frost-rerandomized` **3.0.0**, dikunci persis di `Cargo.lock`.
- Randomizer tetap diambil dari PCZT (`alpha` per spend), lewat API deprecated tersebut. Pemakaian API deprecated dibungkus di satu modul `payout-coordinator::frost` dengan `#[allow(deprecated)]` yang terdokumentasi.
- Risiko bias randomizer oleh coordinator (T-7) tetap dicatat terbuka.

## Alternatif yang ditunda

Alur native v3: jalankan round 1 dulu, turunkan randomizer dari commitments, tulis `alpha` itu ke PCZT, lalu proving. Ini menghapus T-7, tetapi butuh menulis `alpha` ke PCZT sebelum Prover; API PCZT yang mendukungnya belum diverifikasi. Masuk roadmap pasca-MVP.

## Consequences

- Jika versi FROST berikutnya menghapus API deprecated, migrasi ke alur native v3 wajib sebelum upgrade.
- §25, SD-4, dan T-7 diperbarui sesuai keputusan ini.
