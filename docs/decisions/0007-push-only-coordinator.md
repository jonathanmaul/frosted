# ADR-0007: Coordinator push-only, aksi privat lewat `frosted-operator` di localhost

- Status: Accepted
- Tanggal: 2026-10-21
- Bagian spec: §0.2, §6, §13 NFR-S4, §15 UF-2/UF-4, §16 S-11, §17, §18, §19.2, SD-1, SD-3, SD-4, §21, §22, §23.3, §24.2 T-20/T-21, §26, §27.3, §27.4, §30, M5.3, M7.6, M8.6, M11

## Context

Versi 0.1 bertentangan soal arah komunikasi: §22.2 bilang API meneruskan perintah admin ke coordinator, sementara SD-1/3/4 menggambarkan coordinator mendorong data ke API. Host privat butuh Zaino untuk sync escrow, tapi §30 tidak menyediakannya. Endpoint admin di API publik memperluas permukaan serangan dan butuh kunci per member.

## Decision

- Coordinator **tidak menerima koneksi masuk**. Data publik (market, resolusi, vote, plan publik, status tx, progres signing) didorong ke `POST /v1/ingest/*` lewat HTTPS keluar, ditandatangani Ed25519 dengan satu kunci: `COORDINATOR_KEY`.
- Semua aksi privat (create market, vote komite, plan, signing, plan `FOLLOW_UP`) lewat CLI baru `frosted-operator` di host coordinator (localhost). UI lokal opsional.
- `frostd` hanya di localhost. `zebrad` + Zaino milik coordinator berjalan di host privat.
- API publik tidak punya endpoint admin. Kunci admin per member dan kunci operator terpisah dihapus dari spec. Route `/operator` di web publik hanya read-only.
- Opsi pull (member dari pihak berbeda mengirim vote bertanda tangan ke API, coordinator mengambil lewat GET keluar) dicatat sebagai roadmap (Q-4).

## Consequences

- Host privat tidak punya permukaan serangan dari internet (T-21).
- Kebocoran `COORDINATOR_KEY` hanya memungkinkan push palsu, yang tertangkap karena API dan verifier menghitung ulang hash dari chain (T-20).
- Vote komite tidak ditandatangani per member di MVP; keabsahannya bergantung pada coordinator. Ini wajar untuk demo satu orang (ADR-0008) dan harus berubah sebelum komite multi-pihak.
