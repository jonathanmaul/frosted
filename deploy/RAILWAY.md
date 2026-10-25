# Deploy backend testnet di Railway

Status: **file deploy ditulis dan di-build lokal; belum pernah dijalankan di Railway.** Alur yang sama
(tanpa Docker) sudah terbukti di testnet NU7 lewat `scripts/testnet.sh`: market, bet, resolusi, payout
FROST, dan `frosted-verify all` lulus (lihat `docs/TESTING.md` bagian 10).
Web (`web/`) di-deploy ke Vercel; dokumen ini hanya untuk backend.

Tidak ada node sendiri. Backend memakai dua layanan publik:

- **RPC zebrad testnet** (rantai NU7) dari penyedia hosted, mis. Chainstack (paket gratis: 25 request/detik).
  URL-nya berisi kunci akses, jadi **hanya** disimpan sebagai variabel Railway, tidak pernah di repo.
- **lightwalletd publik** `https://testnet.zec.rocks:443` (gRPC TLS) untuk dompet.

## Peta service

```
Vercel (web) ──https──► [public]  frosted-api + glass-indexer   domain publik, volume /data
                            ▲ https, data bertanda tangan Ed25519
                        [coordinator]  payout-coordinator + wallet-helper   tanpa domain, volume /data
                            │ keluar saja: RPC hosted + testnet.zec.rocks
```

| Service | Dockerfile | Context | Domain publik | Volume |
| --- | --- | --- | --- | --- |
| `public` | `deploy/public.Dockerfile` | repo root | **Ya** (port 8080) | `/data` |
| `coordinator` | `deploy/coordinator.Dockerfile` | repo root | **Tidak** | `/data` |

Kenapa digabung:

- `public`: indexer dan API memakai satu file SQLite (`PUBLIC_DB_PATH`), dan volume Railway tidak bisa dibagi antar service.
- `coordinator`: ia menjalankan wallet-helper sebagai subproses. Ia hanya bind loopback dan mendorong data ke API lewat HTTPS, jadi tidak butuh port masuk.

## Variabel per service

Semua service: `RAILWAY_RUN_UID=0` (volume Railway dimiliki root). Cek dokumentasi Railway saat ini bila namanya berubah.

`public`

| Variabel | Nilai |
| --- | --- |
| `FROSTED_NETWORK` | `testnet` |
| `ZEBRA_RPC_URL` | URL RPC hosted (rahasia, berisi kunci) |
| `API_PUBLIC_ORIGIN` | URL web di Vercel, mis. `https://frosted.vercel.app` |
| `COORDINATOR_PUBKEY` | keluaran `frosted-operator secrets init` (langkah 3) |

`coordinator`

| Variabel | Nilai |
| --- | --- |
| `FROSTED_NETWORK` | `testnet` |
| `ZEBRA_RPC_URL` | sama dengan di atas (rahasia) |
| `LIGHTWALLETD_GRPC_URL` | `https://testnet.zec.rocks:443` |
| `API_INGEST_URL` | `https://<domain-api>/v1/ingest` (wajib HTTPS di luar loopback) |
| `TREASURY_ADDRESS` | UA milik Anda yang menerima fee operator (dompet demo, lihat `scripts/testnet.sh`) |

`web` (Vercel): `NEXT_PUBLIC_API_URL=https://<domain-api>/v1`, `NEXT_PUBLIC_NETWORK=testnet`,
`NEXT_PUBLIC_EXPLORER_URL=https://testnet.zecblock.com` (tautan txid dan block di UI).

## Urutan

1. Buat project Railway dan dua service di atas, masing-masing dengan volume `/data`. Beri `public` domain publik.
2. Isi variabel. Simpan URL RPC hanya sebagai variabel Railway.
3. Dari service `coordinator` (`railway ssh`): `frosted-operator secrets init --dir /data/private`. Salin `coordinator_pubkey` ke `COORDINATOR_PUBKEY` di `public`, lalu redeploy `public`.
4. Inisialisasi komite dan wallet hot seperti `scripts/testnet.sh up` (`committee init`, `hot-wallet init`). Danai wallet hot dengan sedikit TAZ dari faucet.
5. Buat market dengan `frosted-operator market create` (butuh TAZ di wallet hot untuk pengumuman FRSTM1). Setelah market tutup dan resolusi masuk: `frosted-operator payout plan`, lalu `payout sign` dengan dua member.
6. Isi variabel Vercel, deploy web, lalu cek `https://<domain-api>/v1/health`.

Indexer dimulai dari tinggi market paling awal. Market yang baru dibuat langsung mengejar tip dalam hitungan detik;
market lama di database lama bisa butuh jam karena RPC hosted dibatasi per detik. Mulai dari volume kosong.

## Belum diketahui / belum diuji

- **Image Docker di Railway.** Kedua image sudah di-build lokal dan `public` lulus smoke test health; `coordinator` belum dijalankan sebagai container.
- **Kuota RPC hosted.** Paket gratis Chainstack 3 juta request unit per bulan dan 25 request per detik. Indexer menyesuaikan diri dengan mencoba ulang saat kena batas, tapi jam sibuk bisa memperlambat sinkron.
- **Biaya dan ukuran Railway.** Kuota, harga, dan batas volume berubah-ubah; cek halaman harga mereka.
- **Keamanan.** Kunci masih trusted dealer: siapa pun yang punya akses dashboard Railway ke service `coordinator` bisa membelanjakan escrow. Hanya testnet.
