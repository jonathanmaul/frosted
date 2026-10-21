# ADR-0012: Deployment laptop-first, snapshot mode di Vercel, tunnel dan VPS opsional

- Status: Accepted, **Amended 2026-10-21** (lihat bagian "Amendment")
- Tanggal: 2026-10-21
- Bagian spec: §18, §24.2 T-8, §25, §26, §27.2, §27.3, §27.4, §27.5, §28 (aturan umum, M12), §30, §33, §34, §36 V-27/V-29

## Context

Spec 0.2 mengasumsikan VPS untuk sisi publik dan host privat terpisah. Developer tidak punya VPS dan bekerja di satu laptop. Juri tetap butuh sesuatu yang bisa dibuka kapan saja, sementara laptop tidak selalu menyala dan sync node testnet/mainnet memakan disk dan waktu.

## Decision

- **Default deployment = laptop lokal.** Sisi publik dan privat berjalan sebagai project Docker Compose terpisah dengan DB terpisah di laptop yang sama. Coordinator tetap tanpa port masuk (ADR-0007). Sisi publik dan privat boleh berbagi satu `zebrad` pada demo.
- **Spike M0 di regtest saja.** Verifikasi pada node live (bagian live M0.5, V-27) ditunda ke M12 dan ditandai "deferred to M12" di `docs/SPIKES.md`. M0.6 memakai server lightwalletd milik Zodl, tanpa node milik developer.
- **Akses testnet disiapkan di M12.0** (lihat Amendment: endpoint publik dulu, node lokal di SSD eksternal sebagai fallback). Kebutuhan disk dan waktu node lokal dicatat di §27.5 (V-29).
- **Demo publik default = video + `frosted-web` di Vercel dalam snapshot mode** (`NEXT_PUBLIC_SNAPSHOT_MODE=true`): read-only, data dari JSON hasil `scripts/export-snapshot.sh` untuk satu market yang sudah `SETTLED`, tanpa node dan tanpa API. Ekspor hanya memakai endpoint publik.
- **Live demo opsional** lewat Cloudflare Tunnel ke `frosted-api` di laptop. README menyatakan bahwa tunnel melihat trafik.
- **VPS opsional**, hanya untuk sisi publik. Sisi privat tidak pernah meninggalkan laptop.

## Consequences

- Pemisahan host publik/privat (§18) di demo hanya berupa pemisahan proses dan DB; dicatat di "Real vs simulated" (§33).
- Vercel dan Cloudflare melihat IP serta request pengunjung, sehingga `LOG_IP=false` tidak berlaku di sana (T-8). Snapshot tidak memuat data bettor yang tidak publik (txid bet, alamat, `env_esk`), dan ini diuji di M12.5.
- Snapshot menunjukkan market yang sudah selesai, bukan odds yang bergerak; gerakan odds ditunjukkan lewat video dan demo live lokal.
- ~~Siklus mainnet mikro (M12.2) butuh node mainnet sendiri (~300 GB, ~3 hari sync).~~ Diganti oleh Amendment di bawah.

## Amendment (2026-10-21)

- **Mainnet mikro diturunkan menjadi opsional (COULD, M13.4).** Dihapus dari metrik wajib §10 dan checklist §33; demo utama = testnet. Tidak ada rencana node mainnet penuh di laptop. Jika mainnet dipakai, evaluasi dulu Zakura dengan pruning/snapshot atau endpoint publik (V-30).
- **Akses testnet di M12 mengutamakan endpoint testnet publik** (JSON-RPC atau lightwalletd-compatible), karena koin testnet tidak bernilai. Sebelum dipakai, verifikasi `getrawtransaction`, `getblock(h, 1)` dengan `valuePools`, `getblockchaininfo`, `sendrawtransaction`, dan lightwalletd dengan data Ironwood (V-31). Fallback: node testnet lokal di SSD eksternal (§27.5).
- Pengecualian terhadap ADR-0007 (coordinator tidak memakai endpoint pihak ketiga) **hanya berlaku untuk testnet**: penyedia endpoint melihat IP laptop, transaksi yang diambil indexer, dan transaksi payout yang disiarkan. Ini dicatat di "Real vs simulated". Untuk mainnet, ADR-0007 tetap berlaku kecuali V-30 memutuskan lain lewat ADR baru.
- Development dan semua spike tetap di regtest.
