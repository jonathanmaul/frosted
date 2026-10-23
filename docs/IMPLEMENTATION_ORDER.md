# Urutan implementasi OpenSpec

Status diperiksa pada 2026-10-23 melalui `openspec list --json`.
Rujukan utama: [PROJECT_SPEC_FROSTED.md §28](PROJECT_SPEC_FROSTED.md#28-implementation-milestones).
Untuk setiap perubahan UI, ikuti [DESIGN.md](DESIGN.md).

## Posisi sekarang

| Change | Selesai | Status |
| --- | ---: | --- |
| `add-glass-wasm` | 11/11 | Selesai |
| `add-frosted-api` | 9/9 | Selesai |
| `add-escrow-markets` | 9/9 | Selesai |
| `add-bettor-web` | 11/12 | Implementasi web selesai; uji Zodl di HP pending |
| `add-resolution-and-payout` | 8/11 | Implementasi tersedia; verifikasi akhir follow-up, checks dan review visual pending |

Alur regtest yang sudah terbukti: buat escrow → push market → announcement →
`OPEN` → bet dari WASM/QR → terindeks `COUNTED` → odds SSE dan My Bets terbarui.
Hasil uji dan batasannya ada di [SPIKES.md](SPIKES.md).

## Urutan berikutnya

| Urutan | Change | Milestone | Task tersisa | Hasil yang dituju |
| --- | --- | --- | ---: | --- |
| 1 | `add-resolution-and-payout` | M8 | 3 | Resolve → plan → FROST 2-of-3 → broadcast → `SETTLED`; payout diterima |
| 2 | `add-persona-thaw` | M9 | 7 | Persona tersimpan terenkripsi, registrasi, Thaw Report, profil dan leaderboard |
| 3 | `add-verifier` | M10 | 8 | CLI dan halaman Verify menghitung ulang data publik serta mendeteksi manipulasi |
| 4 | `add-committee-resolution` | M11 | 5 | Propose/confirm/withdraw, dispute window, resolusi komite atau timeout `VOID` |
| 5 | `prepare-demo-and-docs` | M12 | 13 | Akses testnet, siklus demo nyata, snapshot read-only, README dan video |

### 1. Resolusi dan payout lebih dulu

Ini menutup siklus produk yang masih terputus setelah bet terhitung. Kerjakan
resolver chain, planner dan hash publik, builder PCZT, signing, broadcast/monitor,
lalu tampilan payout. Verifikasi jumlah yang diterima pemenang, fee, refund,
sweep, serta perlindungan terhadap broadcast ganda.

Target keluar: satu siklus regtest lengkap sampai `SETTLED` dengan jumlah payout
yang benar. Perbandingan terhadap verifier penuh dilakukan setelah M10 tersedia;
sebelumnya gunakan perhitungan inti dan test vector bersama.

### 2. Persona dan Thaw

Lengkapi alur Persona yang sekarang baru menerima root lokal di Bet Builder.
Tambahkan penyimpanan terenkripsi, registrasi dan Thaw secara sadar oleh pengguna.
Hasil settlement dari M8 memberi data menang/kalah untuk reputasi.

Target keluar: laporan mencakup menang dan kalah, mengecualikan bet sebelum
registrasi serta tag duplikat, dan root tidak terkirim sebelum pengguna memilih Thaw.

### 3. Verifier

Setelah payout dan fungsi laporan Thaw tersedia, implementasikan pemeriksaan
market, odds, resolusi, plan, Thaw, dan Envelope Proof dengan logika inti bersama.
Verifier CLI memindai node sendiri; hasil endpoint server diberi label sebagai
kemudahan pemeriksaan, bukan audit independen.

Target keluar: PASS untuk data sah dan FAIL dengan diff untuk API yang dimanipulasi.

### 4. Resolusi komite

Tambahkan jalur resolusi alternatif di atas coordinator dan payout M8. Vote dan
bukti terlihat publik, dengan aturan dispute/timeout yang mengikuti tinggi blok.
Setelah selesai, jalankan kembali siklus settlement dan pemeriksaan verifier
untuk market komite.

### 5. Demo dan dokumentasi

Verifikasi kemampuan endpoint testnet terlebih dahulu. Lalu jalankan siklus
testnet, isi bukti transaksi, buat snapshot publik yang aman, dokumentasikan
bagian nyata/simulasi serta batas trust, dan siapkan video/deployment.
Persiapan akses testnet boleh dimulai lebih awal tanpa melewati verifikasi M8–M11.

## Jalur terpisah: uji Zodl

Task `add-bettor-web` 6.2 tetap **PENDING (waiting for operator)**. Pengguna belum
memiliki opsi Zodl testnet di HP. Ini tidak menghalangi M8 dan implementasi lainnya.

Fixture QR/URI testnet serta langkah memeriksa memo dan perubahan amount sudah
disiapkan di [SPIKES.md bagian M0.6](SPIKES.md#m06--s6-zodl-memo), melalui
`node scripts/prepare-zodl-test.mjs` setelah `pnpm build:wasm` di `web/`.
Setelah operator melapor, ambil transaksi dan bandingkan memo byte per byte.
Jangan menandai task selesai sebelum ada bukti atau keputusan fallback §34.
Tidak ada pembayaran mainnet yang diotorisasi untuk uji ini.

## Aturan pengerjaan

- Maksimal **2 sub-agent aktif**, sesuai preferensi pengguna; mulai berurutan
  mengikuti tabel dan paralelkan hanya pekerjaan yang dependensi serta file-nya terpisah.
- Tandai task selesai setelah definition of done terbukti. Perbarui tabel ini
  setelah setiap change agar status tidak tertinggal.
- WASM, API, dan escrow yang selesai belum diarsipkan. Archive adalah perapian
  terpisah dan tidak menjadi syarat memulai M8.
- OpenSpec saat ini diabaikan Git sesuai `.gitignore`; dokumen ini menyimpan
  urutan/status yang ikut repository. Checklist terperinci tetap berada di OpenSpec lokal.
