# Demo E2E di testnet publik (untuk video submission)

Panduan satu siklus penuh Frosted di Zcash testnet (NU7), dijalankan di laptop tanpa node lokal, dengan
bettor memakai **web UI** dan setiap langkah penting bisa dibuktikan di explorer publik
<https://testnet.zecblock.com>. Bagian akhir berisi rencana rekaman untuk diolah menjadi video motion graphics
dengan Remotion.

Status: alur ini sudah terbukti di testnet (market `39e7234e…`, lihat `docs/TESTING.md` §10). Alur UI
yang sama lulus di e2e Playwright regtest (17/17). Perintah `pay` dan `address` di testnet baru; uji pertama
dilakukan saat rekaman.

## 1. Peran dan alat

| Peran | Dikerjakan lewat | Kenapa |
| --- | --- | --- |
| Bettor: pilih outcome, jumlah, alamat payout, segel bet | Browser, `http://localhost:3100` | Bet dibangun di browser (WASM); rahasia tidak keluar dari perangkat |
| Dompet bettor: membayar URI ZIP-321 | `scripts/testnet.sh pay '<uri>'` (dompet `demo`, zcash-devtool) | Pengganti aplikasi dompet di HP; membayar URI tanpa mengubahnya |
| Operator: membuat market, menandatangani payout | Terminal (`scripts/testnet.sh`, `frosted-operator`) | Kunci FROST tidak boleh ada di browser; halaman `/operator` hanya baca |
| Siapa pun: memeriksa | Halaman market, halaman Verify, explorer | Bukti on-chain dan verifikasi ulang |

Yang **tidak** ditampilkan di UI, dan memang disengaja: txid bet (privasi, spec melarang). Bukti bet di UI adalah
tinggi block di feed anonim. Txid pengumuman market dan txid payout ditampilkan dan bisa dibuka di explorer.

## 2. Persiapan (sekali, sebelum hari rekaman)

- [ ] `~/.config/frosted/testnet.env` berisi `ZEBRA_RPC_URL=...` (RPC Chainstack). File ini di luar repo; jangan
      ditampilkan di layar saat merekam.
- [ ] Dompet `demo` ada di `.regtest/testnet-demo` dan punya saldo. Butuh kira-kira 0,03 TAZ bebas
      (2 bet × 0,01 + biaya). Biaya bersih satu siklus terakhir sekitar 0,00025 TAZ karena payout kembali ke dompet ini.
- [ ] Docker jalan (zcash-devtool berjalan di container).
- [ ] `cd web && pnpm install` sudah pernah dijalankan.
- [ ] Siapkan dua jendela terminal (operator dan dompet) dan satu jendela browser bersih (tanpa ekstensi yang
      menutupi layar; zoom 100%).

## 3. Jalankan stack (operator)

```bash
scripts/testnet.sh fresh     # simpan state lama ke .testnet/old-<waktu>/, indexer mulai dari tip
scripts/testnet.sh up        # kunci, komite 2-of-3, hot wallet; indexer, API, coordinator
scripts/testnet.sh status    # proses, tinggi block, lag indexer, daftar market
```

`fresh` wajib bila state lebih tua dari beberapa jam. Tanpa itu indexer harus mengejar ribuan block lewat RPC yang
dibatasi (sekitar 0,3 block/detik), bisa berjam-jam.

Pengumuman market dibayar dari hot wallet. Kalau `market` gagal karena saldo, isi dengan
`scripts/testnet.sh fund 200000` (dari dompet demo).

## 4. Buat market (operator)

```bash
scripts/testnet.sh market 30 "Will the Ironwood pool hold at least 1 ZEC?"
```

- Tutup 30 block setelah pengumuman (sekitar 30 menit; block testnet kira-kira 1 per menit, kadang macet).
- Simpan `market_id` yang dicetak; perintah lain memakai market terakhir bila id tidak diberikan.
- Status `ANNOUNCING` berubah menjadi `OPEN` setelah transaksi pengumuman (FRSTM1) ditambang.

## 5. Buka web dan siapkan alamat payout

```bash
scripts/testnet.sh web       # http://localhost:3100, link explorer sudah aktif
scripts/testnet.sh address   # alamat dompet demo; salin untuk kolom Payout Address
```

Memakai alamat dompet demo sebagai alamat payout membuat kemenangan kembali ke dompet yang sama, jadi TAZ tidak hilang.

## 6. Bettor: dua bet lewat browser

Ulangi dua kali: sekali YES, sekali NO (supaya ada pihak menang dan kalah).

1. Buka `http://localhost:3100/markets`, klik market baru, tunggu label `LIVE` di panel Public odds.
2. Bet Builder: **01** pilih YES → Continue; **02** isi `0.01` → Continue; **03** tempel alamat dari
   `address` → Continue; **04** periksa ringkasan → **Seal & save receipt**.
3. Muncul QR dan URI `zcash:...`. Klik **Copy URI**.
4. Di terminal dompet:

   ```bash
   scripts/testnet.sh pay 'zcash:utest1...?amount=0.01&memo=...'
   ```

   Perintah mencetak txid pembayaran (itu milik dompet Anda; Frosted tidak menampilkannya).
5. Kembali ke browser. Setelah sekitar 1 menit (satu block) pool di halaman market bertambah tanpa reload; feed
   anonim menampilkan jumlah, outcome, dan tinggi block.
6. Buka **My Bets**: receipt tersimpan di browser; status berubah dari `WAITING` ke `COUNTED` setelah konfirmasi.

Ulangi untuk NO. Satu sesi browser cukup; receipt kedua juga masuk My Bets.

## 7. Tunggu tutup dan resolusi

- Market menutup di `close_height`, lalu resolusi `POOL_THRESHOLD` dipublikasikan **otomatis** setelah
  `resolve_height` ditambah konfirmasi minimum. Status: `OPEN` → `CLOSED` → `RESOLVED`.
- Total dari pengumuman sampai siap payout terakhir kali sekitar 35 block (45–75 menit tergantung kecepatan block).
  Potong bagian menunggu di video.
- Pantau di halaman market (bagian Resolution & payout) atau `scripts/testnet.sh status`.

## 8. Payout FROST 2-of-3 (operator)

```bash
scripts/testnet.sh payout <market_id>
```

Membuat rencana payout (plan_hash), anggota 1 dan 2 menyetujui dengan mengetik plan_hash, menandatangani, dan
menyiarkan. Txid payout dicetak. Setelah ditambang, status market menjadi `SETTLED`.

## 9. Bukti on-chain dan verifikasi

1. Halaman market → panel **On-chain proof**:
   - Market announced: txid pengumuman + block → **View on explorer**.
   - Payouts: txid payout → **View on explorer**.
   - Feed anonim: tinggi block setiap bet, bisa diklik ke halaman block di explorer.
2. Di explorer, tunjukkan transaksi ada dan kapan ditambang. Jumlah dan pihak tersembunyi (shielded); itu poinnya.
3. Halaman **Verify** (`/verify/<market_id>`): perintah `frosted-verify` untuk menghitung ulang semuanya dari chain.
   Opsional di terminal:

   ```bash
   source ~/.config/frosted/testnet.env   # jangan tampilkan isinya di layar
   target/debug/frosted-verify all <market_id> --node "$ZEBRA_RPC_URL" --api http://127.0.0.1:28080/v1
   ```

4. **My Bets**: status bet pemenang berubah sesuai payout. Payout masuk ke alamat dompet demo.

## 10. Selesai

```bash
scripts/testnet.sh down
```

## 11. Masalah yang mungkin muncul

| Gejala | Penyebab | Tindakan |
| --- | --- | --- |
| Menara odds dan `BLOCK —` abu-abu | API belum jalan atau belum ada market OPEN | Jalankan `up`, buat market, tunggu OPEN |
| `indexer scanning: block X of T` lama | State lama | `down`, `fresh`, `up` |
| `pay` gagal: saldo | Dompet demo kurang | Isi dari faucet ke alamat Orchard (faucet menolak UA dengan receiver Ironwood) |
| Market lama tidak bisa dibet | Sudah `SETTLED` | Buat market baru |
| Status tidak berubah beberapa menit | Block testnet macet | Tunggu; cek tinggi block di zecblock |
| Bet Builder: `PAYOUT_ADDRESS_NO_ORCHARD` | Alamat bukan UA dengan Orchard | Pakai keluaran `scripts/testnet.sh address` |

## 12. Rencana rekaman untuk video (Remotion)

Panduan produksi lengkap (naskah narasi, nama file klip, setup Remotion, prompt Claude Code per adegan, render,
cek kualitas) ada di `docs/VIDEO-REMOTION.md`. Ringkasnya: rekam layar mentah per adegan, lalu susun di
Remotion. Saran rekaman: 1920×1080, 30–60 fps, terminal font ≥ 18 pt, browser zoom 100%. Simpan klip di
`frosted-video/public/clips/` (proyek Remotion di luar repo ini). Tabel di bawah adalah gambaran adegan;
nama file klip yang dipakai prompt mengikuti `docs/VIDEO-REMOTION.md` §3.

| # | Adegan | Sumber | Durasi akhir | Overlay |
| --- | --- | --- | --- | --- |
| 01 | Landing: scroll Through the Frost | Browser | 12 s | Judul "Public odds. Invisible bettors." |
| 02 | Operator membuat market | Terminal | 8 s | "Rules locked on-chain (FRSTM1)" |
| 03 | Market OPEN, odds live | Browser | 6 s | Panah ke label LIVE |
| 04 | Bet Builder 4 langkah + QR | Browser | 15 s | "Built in the browser. Secrets stay local." |
| 05 | `pay` dari dompet | Terminal | 6 s | "Any Zcash wallet can pay this URI" |
| 06 | Pool bertambah live, feed anonim | Browser | 8 s | "Amounts visible. Senders shielded." |
| 07 | My Bets COUNTED | Browser | 5 s | — |
| 08 | Lompatan waktu ke RESOLVED | Browser | 4 s | Penghitung block cepat (dibuat di Remotion) |
| 09 | Payout 2-of-3 | Terminal | 8 s | Diagram 3 penanda, 2 menyala (dibuat di Remotion) |
| 10 | On-chain proof → explorer | Browser | 12 s | Sorot txid; zoom ke halaman zecblock |
| 11 | Verify | Browser/terminal | 8 s | "Recompute everything from the chain" |
| 12 | Penutup | Remotion | 6 s | Logo `web/public/logo/frosted-logo.png`, tagline |

Aset yang bisa dipakai ulang di Remotion: logo (`web/public/logo/`), palet Paper (`docs/DESIGN.md` §14), font
Instrument Serif dan IBM Plex (paket `@fontsource`), dan screenshot halaman.
