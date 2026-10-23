# Deploy backend testnet di Railway

Status: **file deploy ditulis dan di-build lokal, belum pernah dijalankan di testnet atau di Railway.**
Web (`web/`) di-deploy ke Vercel; dokumen ini hanya untuk backend.

## Peta service

```
Vercel (web) ──https──► [public]  frosted-api + glass-indexer   domain publik, volume /data
                            ▲ https, data bertanda tangan Ed25519
                        [coordinator]  payout-coordinator + wallet-helper   tanpa domain, volume /data
                            │ jaringan privat                    │ jaringan privat
                            └──────────► [node]  zebrad + zainod ◄┘  tanpa domain, volume /data
```

| Service | Dockerfile | Context | Domain publik | Volume |
| --- | --- | --- | --- | --- |
| `node` | `deploy/node/Dockerfile` | `deploy/node` | **Tidak** | `/data` (disk besar) |
| `public` | `deploy/public.Dockerfile` | repo root | **Ya** (port 8080) | `/data` |
| `coordinator` | `deploy/coordinator.Dockerfile` | repo root | **Tidak** | `/data` |

Kenapa digabung:

- `node`: Zaino menolak alamat validator non-localhost tanpa TLS, jadi zebrad dan zainod satu container.
- `public`: indexer dan API memakai satu file SQLite (`PUBLIC_DB_PATH`), dan volume Railway tidak bisa dibagi antar service.
- `coordinator`: ia menjalankan wallet-helper sebagai subproses. Ia hanya bind loopback dan mendorong data ke API lewat HTTPS, jadi tidak butuh port masuk.

## Variabel per service

Semua service: `RAILWAY_RUN_UID=0` (volume Railway dimiliki root). Cek dokumentasi Railway saat ini bila namanya berubah.

`public`

| Variabel | Nilai |
| --- | --- |
| `FROSTED_NETWORK` | `testnet` |
| `ZEBRA_RPC_URL` | `http://node.railway.internal:18232` lihat catatan RPC di bawah |
| `API_PUBLIC_ORIGIN` | URL web di Vercel, mis. `https://frosted.vercel.app` |
| `COORDINATOR_PUBKEY` | keluaran `frosted-operator secrets init` (langkah 4) |

`coordinator`

| Variabel | Nilai |
| --- | --- |
| `FROSTED_NETWORK` | `testnet` |
| `ZEBRA_RPC_URL` | sama dengan di atas |
| `LIGHTWALLETD_GRPC_URL` | `http://node.railway.internal:8137` |
| `API_INGEST_URL` | `https://<domain-api>/v1/ingest` (wajib HTTPS di luar regtest) |
| `TREASURY_ADDRESS` | UA Orchard-only milik Anda (**belum ditentukan, lihat "Belum diketahui"**) |

`web` (Vercel): `NEXT_PUBLIC_API_URL=https://<domain-api>/v1`, `NEXT_PUBLIC_NETWORK=testnet`.

## Urutan

1. Buat project Railway dan service `node`. Pasang volume di `/data`. **Deploy dulu dan biarkan sinkron** (lama; lihat log sampai tinggi block mendekati tip testnet).
2. Buat service `public` dan `coordinator` dengan variabel di atas. Beri `public` domain publik.
3. Dari service `coordinator` (`railway ssh`): `frosted-operator secrets init --dir /data/private`. Salin `coordinator_pubkey` ke `COORDINATOR_PUBKEY` di `public`, lalu redeploy `public`.
4. Inisialisasi komite dan Persona Registry seperti `scripts/dev.sh` (`committee init`, `glass-indexer registry-set`). Lihat `scripts/dev.sh` bagian `init_state` dan `start_daemons` untuk argumen persisnya.
5. Buat market lewat `frosted-operator market create` (butuh ZEC testnet di wallet hot untuk pengumuman FRSTM1).
6. Isi variabel Vercel, deploy web, lalu cek `https://<domain-api>/v1/health`.

## Belum diketahui / belum diuji

- **Ironwood (NU6.3) di testnet asli.** Seluruh proyek diuji di regtest yang mengaktifkan NU6.3 pada block 10. Apakah testnet publik sudah mengaktifkannya, dan apakah alamat/memo yang dipakai kode berlaku di sana, belum diverifikasi. Cek dulu sebelum menghabiskan waktu sinkron.
- **Wallet-helper dan coordinator di testnet.** Cabang jaringan `testnet` ada di kode tetapi belum pernah dijalankan.
- **`TREASURY_ADDRESS` dan wallet hot.** Di dev skrip memakai alamat Persona Registry. Untuk testnet perlu alamat dan dana sendiri; caranya belum ditulis.
- **Akses RPC zebrad.** RPC zebrad diikat ke `0.0.0.0:18232` agar service `public` dan `coordinator` bisa menjangkaunya lewat jaringan privat Railway. Ini aman hanya selama service `node` tidak pernah diberi domain publik.
- **Biaya dan ukuran.** Kuota, harga, dan batas volume Railway berubah-ubah; cek halaman harga mereka.
- **Keamanan.** Kunci masih trusted dealer: siapa pun yang punya akses dashboard Railway ke service `coordinator` bisa membelanjakan escrow. Hanya testnet.
