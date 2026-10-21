# ADR-0014: Signing FROST in-process di `frosted-operator` untuk MVP

- Status: Accepted
- Tanggal: 2026-10-21
- Bagian spec: §18, §22.2, §23.3 J-9, §26, §27.1, §30, SD-4, M8.4
- Bukti: `docs/SPIKES.md` §M0.4

## Context

`frostd`/`frost-client` dari `frost-zcash-demo` (commit `53df99c`) masih memakai FROST 2.x, sehingga key package-nya tidak kompatibel dengan FROST v3 yang dipakai MVP (ADR-0013). Di demo MVP, ketiga share dipegang satu orang (ADR-0008), dan coordinator tidak menerima koneksi masuk (ADR-0007).

## Decision

- Untuk MVP, ronde FROST (commitment, signature share, agregasi) dijalankan **in-process** oleh `frosted-operator` di host coordinator, memakai share yang disimpan terpisah per member (satu file terenkripsi per share).
- `frostd`/`frost-client` tidak dipakai di MVP.
- Signer tetap melihat `plan_hash` dan ringkasan plan sebelum share-nya dipakai (NFR-S3): `frosted-operator payout sign` menampilkan keduanya dan meminta konfirmasi per member.

## Alternatif yang ditunda

Port `frostd`/`frost-client` ke FROST v3 untuk komite multi-pihak sungguhan (roadmap, bersama opsi pull Q-4).

## Consequences

- Tidak ada server `frostd` di stack privat. Konfigurasi `FROSTD_URL` dihapus dari §27.4.
- "Real vs simulated" (§33) mencatat bahwa signing 2-of-3 berjalan dalam satu proses.
- Waktu signing terukur 2–7 ms per sesi (in-process), sehingga satu sesi per spend tidak menjadi hambatan.
