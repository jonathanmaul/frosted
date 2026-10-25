# TESTING.md: panduan menguji Frosted sendiri

Panduan ini untuk menjalankan dan menguji seluruh Frosted di mesin sendiri, dari nol sampai skenario manual per fitur.
Semua dijalankan di **regtest** (chain buatan di Docker): tidak memakai uang sungguhan, akun luar, atau HP.

Status per 2026-10-23: sudah teruji di regtest. **Belum** teruji: Zodl di HP, testnet, mainnet (lihat §9).

## 1. Peta singkat

```
Browser ── web (Next.js :3000) ── frosted-api (:8080, publik) ── public.db ── glass-indexer ──┐
                                        ▲ push bertanda tangan                                │ baca chain
                                        │                                                     ▼
frosted-operator (CLI) ── payout-coordinator (:8090, loopback, privat) ── wallet-helper ── zebrad + Zaino (Docker)
frosted-verify (CLI) ─────────────────────────────── membaca node sendiri + API ───────────────┘
```

- Tidak ada smart contract: escrow adalah alamat shielded dengan kunci threshold FROST 2-of-3, aturan dihitung di luar chain dan dapat diaudit dengan `frosted-verify`.
- Di demo satu orang memegang ketiga share komite. Jendela sengketa dan "komite" adalah simulasi.

## 2. Yang harus disiapkan (sekali)

### 2.1 Perangkat

| Kebutuhan | Cek | Catatan |
| --- | --- | --- |
| macOS/Linux, ±15 GB disk kosong | | Docker image, build Rust, `.regtest/` |
| Docker (daemon jalan) | `docker ps` | Image zebra dan zaino dibangun otomatis saat pertama |
| Rust (rustup) | `cargo --version` | `rust-toolchain.toml` memilih versinya dan target `wasm32` |
| `wasm-pack` | `wasm-pack --version` | `cargo install wasm-pack` |
| Node.js 24+ dan `pnpm` | `node -v && pnpm -v` | |
| `sqlite3`, `curl`, `python3`, `lsof` | | Dipakai skrip |
| Port kosong | `lsof -iTCP:3000 -iTCP:8080 -iTCP:8090 -sTCP:LISTEN` | 3000 web, 8080 API, 8090 coordinator. Stack Docker memakai 18232 dan 9067 |

### 2.2 Langkah persiapan

```bash
cd frosted

# 1. Dependensi web + browser untuk tes E2E
cd web && pnpm install && pnpm exec playwright install chromium && cd ..

# 2. WASM (Bet Builder dan Persona berjalan di browser)
cd web && pnpm build:wasm && cd ..

# 3. Wallet helper (stack wallet pre-release, workspace terpisah). Di Mac dengan protoc Homebrew rusak,
#    pakai PATH di bawah (lihat wallet-helper/README.md)
env PATH="$HOME/.cargo/bin:/usr/bin:/bin" cargo build --release --locked --manifest-path wallet-helper/Cargo.toml

# 4. Node regtest (zebra + Zaino)
scripts/dev-up.sh

# 5. Wallet uji `faucet` (didanai) dan `recipient` (alamat payout). Sekali saja, ±beberapa menit
scripts/regtest-wallet.sh setup
```

Cek hasilnya:

```bash
curl -s -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"getblockcount","params":[]}' http://127.0.0.1:18232
scripts/regtest-wallet.sh dt wallet -w /work/faucet balance      # saldo faucet harus > 0
ls .regtest/faucet .regtest/recipient                            # kedua wallet ada
```

> Jangan jalankan `docker compose down -v`: itu menghapus chain dan wallet yang sudah terisi, dan Anda harus mengulang langkah 5.

### 2.3 Yang TIDAK perlu Anda siapkan

Wallet HP, akun cloud, API key, ZEC testnet atau mainnet, domain, atau Vercel. Semuanya hanya dibutuhkan di tahap demo (lihat §9).

## 3. Menjalankan seluruh stack: satu perintah

```bash
scripts/dev.sh
```

Perintah ini menyalakan zebra+Zaino (jika belum), membangun binary, lalu menjalankan indexer, API, coordinator, dan web. Run pertama juga membuat keys, komite 2-of-3, hot wallet berisi dana, dan **dua market demo** (satu chain-verified, satu komite). Selesai bila muncul ringkasan:

```
Web           http://localhost:3000
API           http://localhost:8080/v1
Coordinator   http://127.0.0.1:8090
```

`Ctrl-C` mematikan semuanya. Perintah lain:

| Perintah | Fungsi |
| --- | --- |
| `scripts/dev.sh up --detach` | Jalan di background |
| `scripts/dev.sh status` | Proses, URL, daftar market |
| `scripts/dev.sh down` | Matikan daemon dan web (Docker tetap hidup) |
| `scripts/dev.sh reset` | Hapus state `.regtest/dev` (keys, DB, market). Chain tetap |
| `scripts/dev.sh pay '<zcash:...>'` | Bayar URI dari Bet Builder atau Persona memakai faucet, lalu tambang 3 blok |
| `scripts/dev.sh mine 10` | Tambang 10 blok (menggerakkan close, resolve, dispute window) |
| `scripts/dev.sh recipient` | Alamat payout untuk bet uji |
| `scripts/dev.sh op <args>` | `frosted-operator` untuk stack ini |
| `scripts/dev.sh logs web\|api\|indexer\|coordinator` | Ekor log |

Opsi lewat environment: `DEV_STATE`, `WEB_MODE=prod`, `API_PORT`, `WEB_PORT`, `COORD_PORT`, `DEV_SEED=0` (tanpa market demo), `DEV_CLOSE_IN=120` (blok sampai market tutup).

Log dan state ada di `.regtest/dev/` (`logs/*.log`).

### Jika ada yang gagal

| Gejala | Penyebab dan solusi |
| --- | --- |
| `port 8080 is already in use` | Ada proses lain. `scripts/dev.sh down` atau matikan prosesnya |
| `market create` gagal sekali lalu berhasil | Zaino telat mengindeks; skrip mengulang otomatis (aman, tidak menulis apa pun saat gagal) |
| Market tidak jadi `OPEN` | Announcement butuh blok: `scripts/dev.sh mine 3`, tunggu ±10 detik |
| Web menampilkan "Markets unavailable" | API belum jalan atau port salah: `scripts/dev.sh status` |
| `wallet helper failed` di log coordinator | `cat .regtest/dev/logs/helper.log` (hanya nama op dan kode keluar; alasan disembunyikan sengaja) |
| `already locked` | Dua coordinator memakai DB yang sama. Matikan yang lama |

## 4. Tes otomatis

### 4.1 Unit dan integrasi (tanpa chain)

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace            # ±146 tes
cd web && pnpm lint && pnpm exec tsc --noEmit && pnpm test     # ±73 tes
```

### 4.2 Skrip regtest (chain nyata, daemon sendiri, tiap skrip mandiri)

Butuh §2 selesai dan `scripts/dev-up.sh` jalan. Tiap skrip memakai port 18xxx dan state sendiri, jadi aman walau `scripts/dev.sh` sedang jalan.

| Skrip | Menguji | Lama |
| --- | --- | --- |
| `scripts/it-indexer.sh` | Indexer: klasifikasi bet, dust, reorg, VOID | ±3 mnt |
| `scripts/it-persona-thaw.sh` | Persona terdaftar on-chain, bet ber-tag, report Thaw (menang dan kalah, `PRE_REGISTRATION`, `DUPLICATE_TAG`) | ±1 mnt |
| `scripts/it-verify.sh` | `frosted-verify`: PASS pada data jujur, FAIL (exit 1) saat dimanipulasi, `/verify` sama dengan CLI | ±1 mnt |
| `scripts/it-committee.sh` | Siklus penuh COMMITTEE → payout FROST 2-of-3 → `SETTLED`, plus VOID tanpa keputusan | ±4 mnt |

Keluaran sukses: baris `PASS ...` lalu `==> all ... passed`. Jika `FAIL`, baca pesannya dan `.regtest/<nama>/` (log daemon).

### 4.3 Tes browser nyata (Playwright)

```bash
scripts/e2e.sh                                 # semua spec (17 tes, ±5-6 mnt termasuk menyalakan stack)
scripts/e2e.sh e2e/02-bet.spec.ts              # satu spec
cd web && pnpm exec playwright show-report     # laporan HTML
```

`e2e.sh` membangun stack **baru** terisolasi (`.regtest/e2e`, port 13000/18080/18090, build Next `.next-e2e`), menjalankan tes, lalu mematikannya. Screenshot tiap alur ada di `web/test-results/shots/`.

| Spec | Isi |
| --- | --- |
| 01 | Daftar market, dropdown kustom (klik, keyboard), tidak ada request keluar, layar mobile, halaman market |
| 02 | Bet nyata: WASM, receipt lokal, pembayaran, pool bertambah live, My Bets "Counted" |
| 03 | Persona: buat, enkripsi, registrasi on-chain, bet ber-tag, Thaw, profil, leaderboard |
| 04 | Security header + CSP, dan halaman Verify |
| 05 | Komite: propose, confirm, withdraw, resolve |

Spec berjalan berurutan dan menggerakkan chain (urutan penting: 05 menambang melewati `close_height`).

## 5. Skenario uji manual (lewat UI)

Jalankan `scripts/dev.sh` lalu buka http://localhost:3000. Untuk tiap langkah "bayar", salin **Payment URI** dari halaman dan jalankan `scripts/dev.sh pay '<uri>'` di terminal lain.

### 5.1 Melihat market (S-01, S-02)
1. Buka **Markets**. Harus ada 2 market `OPEN`.
2. Filter **Status** dan **Kind**: klik dropdown, pilih opsi; coba keyboard (↑ ↓ Home End Enter Esc, ketik huruf).
3. Buka market chain-verified. Harus tampil: odds (— sebelum ada bet), "0.00 ZEC in the pool", label `LIVE`, aturan terkunci, `rules_hash`.

### 5.2 Bet (S-03, S-04)
1. Di halaman market: pilih outcome → Continue → jumlah (min 0.01) → Continue.
2. Payout address: isi hasil `scripts/dev.sh recipient`. Continue → **Seal & save receipt**.
3. Muncul QR dan URI. Bayar: `scripts/dev.sh pay '<uri>'`.
4. **Harapan**: tanpa reload, pool bertambah (SSE). Buka **My Bets** → Refresh statuses → status `Counted`.
5. Negatif: alamat payout ngawur → pesan error, tidak ada QR. Jangan ubah jumlah di wallet: jumlah berbeda menandai bet `Amount changed`.

### 5.3 Persona dan Thaw (S-06 sampai S-09)
1. **Persona** → Create Persona. Catat backup key (64 hex). Passphrase terkunci sampai kotak "I saved this key" dicentang. Isi passphrase ≥ 12 karakter → Encrypt and continue.
2. Muncul QR registrasi. Bayar dengan `scripts/dev.sh pay '<uri>'`. **Harapan**: dalam ±10-20 detik muncul "Registered at block N".
3. Buat bet di market, centang **Tag this bet with my Persona**, isi passphrase, bayar. Bet publik harus bertanda `tagged`.
4. **Thaw**: isi alias, ulangi alias, passphrase. Tombol aktif hanya jika alias sama persis. Setelah Thaw: laporan menampilkan semua bet ber-tag (termasuk yang kalah atau belum selesai) dan kuncinya menjadi publik.
5. Buka `/p/<persona_id>` dan **Leaderboard**. Badge `10+` hanya untuk ≥ 10 bet terselesaikan.
6. Negatif: passphrase salah → "Wrong passphrase", tidak ada yang terkirim. Thaw kedua → ditolak (`PERSONA_ALREADY_THAWED`).

### 5.4 Komite (S-05, S-11)
1. Buka market komite: tampil "No proposal yet" dan hitung mundur void.
2. Gerakkan chain melewati `resolve_height` (lihat halaman): `scripts/dev.sh mine 130`.
3. Vote lewat CLI:
   ```bash
   ID=<market_id>; U=https://example.org/e
   scripts/dev.sh op resolution propose $ID --outcome 0 --evidence-url $U --member member-1
   scripts/dev.sh op resolution confirm $ID --outcome 0 --evidence-url $U --member member-2
   ```
   **Harapan** (±5 detik, reload halaman): status `PROPOSED`, dua vote, "Dispute window ends at block N: 6 blocks left".
4. `scripts/dev.sh op resolution withdraw $ID --member member-3` → kembali `AWAITING_RESOLUTION`, vote kosong.
5. Propose + confirm lagi, lalu `scripts/dev.sh mine 8` → `RESOLVED`.
6. Negatif: confirm dengan outcome berbeda → `COMMITTEE_MISMATCH`; member sama → `COMMITTEE_SAME_MEMBER`; propose sebelum `resolve_height` → `COMMITTEE_NOT_OPEN`.
7. Void: market komite tanpa keputusan menjadi `VOID` setelah `void_after_blocks` (60 untuk demo).

### 5.5 Resolusi chain-verified dan payout
1. Setelah ada bet, tambang melewati `resolve_height` + 3: coordinator menerbitkan resolusi otomatis.
2. Rencana payout dan tanda tangan FROST (dua member):
   ```bash
   scripts/dev.sh op payout plan $ID                      # catat plan_hash
   S=.regtest/dev
   printf '%s\n%s\n' <hash> <hash> | scripts/dev.sh op payout sign $ID --plan-hash <hash> \
     --share $(ls $S/shares/*.share.enc | sed -n 1p) --member-key $S/private/member-1.key \
     --share $(ls $S/shares/*.share.enc | sed -n 2p) --member-key $S/private/member-2.key
   scripts/dev.sh mine 10
   ```
   Pembuatan proof memakan waktu; tunggu perintah selesai. **Harapan**: market `PAYING` lalu `SETTLED`, payout muncul di S-05 dan wallet `recipient`.
3. Cek diri sendiri: lihat §5.6.

### 5.6 Verifier (S-10 dan CLI)
```bash
target/debug/frosted-verify all <market_id> --node http://127.0.0.1:18232 --api http://localhost:8080/v1
```
- **Harapan**: `[PASS]` untuk rules_hash, alamat, pengumuman, pools, resolusi, dan tiap plan; `RESULT: PASS`, exit 0.
- Di web: `/verify/<id>` → **Run server-side check** (hanya kemudahan, tidak boleh dipercaya).
- Uji manipulasi: ubah salinan DB lalu jalankan lagi, harus `FAIL` dengan diff dan exit 1. Contoh otomatisnya di `scripts/it-verify.sh`.
- `--json` untuk keluaran mesin; `--help` untuk semua opsi.

### 5.7 Envelope Proof (membantah label UNCLAIMABLE)
Ekspor dari My Bets (kartu bet → Export Envelope Proof), simpan sebagai `proof.json`:
```bash
target/debug/frosted-verify envelope-proof proof.json --node http://127.0.0.1:18232 --api http://localhost:8080/v1
```
Proof asli → `PASS`; proof palsu atau diubah → `FAIL`.

## 6. Yang perlu dicermati saat menguji (ceklis cepat)

- [ ] Tidak ada `<select>` bawaan browser di UI; dropdown bisa dipakai dengan keyboard.
- [ ] DevTools → Network: semua request hanya ke `localhost:3000` dan `localhost:8080`. Tidak ada font atau CDN luar.
- [ ] DevTools → Application → IndexedDB `frosted`: kunci Persona hanya ada sebagai ciphertext. (Bet receipt berisi alamat payout dan `env_esk` plaintext: disengaja, ADR-0006.)
- [ ] Response API tidak memuat txid bet, alamat penerima, atau memo mentah.
- [ ] Header respons halaman memuat `Content-Security-Policy`, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`.
- [ ] Layar sempit (DevTools → 390px): tidak ada scroll horizontal.
- [ ] Setiap status punya teks (bukan hanya warna).

## 7. Reset dan pembersihan

```bash
scripts/dev.sh down && scripts/dev.sh reset     # state stack utama
rm -rf .regtest/e2e .regtest/committee-run .regtest/it-*   # sisa skrip uji
```
Chain regtest, wallet `faucet` dan `recipient` tetap ada. Mengosongkan chain total: `docker compose -f infra/docker-compose.yml down -v`, lalu ulangi §2.2 langkah 4-5.

## 8. Referensi

| Dokumen | Isi |
| --- | --- |
| `docs/PROJECT_SPEC_FROSTED.md` | Spesifikasi lengkap, alur pengguna (UF-1 sampai UF-5), layar, API |
| `docs/DESIGN.md` | Brief visual dan daftar yang dilarang |
| `docs/SPIKES.md` | Bukti teknis per milestone, termasuk M0.6 Zodl |
| `docs/IMPLEMENTATION_ORDER.md` | Status change dan urutan berikutnya |
| `docs/decisions/` | ADR (mis. 0006 receipt, 0007 push-only, 0008 kustodian) |
| `wallet-helper/README.md` | Cara build helper dan protokol JSON-nya |

## 9. Belum teruji dan batasan yang jujur

| Hal | Status |
| --- | --- |
| **Zodl di HP** | Task `add-bettor-web` 6.2 masih PENDING. Zodl tidak bisa menjangkau regtest, jadi butuh Zodl dengan opsi **testnet** dan ZEC testnet. Fixture QR dan langkah cek memo: `node scripts/prepare-zodl-test.mjs` (setelah `pnpm build:wasm`), lalu `docs/SPIKES.md` bagian M0.6 |
| **Testnet / mainnet** | Testnet: terbukti end-to-end lewat `scripts/testnet.sh` (bagian 10). Mainnet di luar cakupan |
| **Kustodian tunggal** | Satu orang memegang ketiga share komite (ADR-0008); vote berupa label, bukan tanda tangan (ADR-0007). Jendela sengketa hanyalah simulasi |
| **Kriptografi** | Belum diaudit independen. `cargo audit` belum dipasang |
| **Browser** | Hanya Chromium yang diuji otomatis (Safari/Firefox belum) |
| **Rate limit API** | Belum ada (selain cache `/verify`). Alias Persona belum dijamin unik |
| **Keamanan CSP** | `script-src` memakai `'unsafe-inline'` (bootstrap Next). Nonce per request belum dibuat |
| **Dev mode** | CSP `next dev` lebih longgar dan belum diuji; yang diuji adalah build produksi |

## 10. Testnet tanpa node sendiri (status 2026-10-25)

Frosted jalan di testnet publik (NU7) tanpa node lokal: RPC dari endpoint pihak ketiga, dompet ringan lewat `testnet.zec.rocks`.

**Persiapan sekali:**
1. Endpoint RPC testnet **di chain NU7** (mis. node Chainstack paket gratis). Simpan di `~/.config/frosted/testnet.env` sebagai `ZEBRA_RPC_URL=https://...`, di luar repo. Jangan commit dan jangan tempel di chat.
2. Dompet demo: `zcash-devtool wallet init --name demo -n test` di `.regtest/testnet-demo` (folder ini diabaikan git), lalu isi dari faucet (mis. faucet.testnet.valargroup.dev, 0,125 TAZ per hari). Faucet menolak alamat yang membawa receiver Ironwood ("the wallet cannot pay this address"); buat alamat Orchard-only dengan `wallet generate-address -p orchard`.
3. Explorer yang benar: https://testnet.zecblock.com (`/tx/<txid>`, `/block/<tinggi>`). `testnet.zcashexplorer.app` berada di chain lama sebelum NU7 dan tidak akan menemukan transaksi kita.

**Alur demo lengkap (`scripts/testnet.sh`):**

```
scripts/testnet.sh fresh                # sekali, bila ada state lama: pindahkan market lama agar indexer mulai dari tip
scripts/testnet.sh up                   # kunci, komite, dompet hot; indexer + API + coordinator
scripts/testnet.sh fund 3000000         # kirim 0,03 TAZ dari dompet demo ke dompet hot (bila perlu)
scripts/testnet.sh market 30            # market chain-verified, tutup sekitar 30 blok (sekitar 40 menit)
scripts/testnet.sh bet <market_id> 0 1000000    # bet YES 0,01 TAZ (min bet)
scripts/testnet.sh bet <market_id> 1 1000000    # bet NO
scripts/testnet.sh web                  # web di http://localhost:3100/markets, tautan ke testnet.zecblock.com
# setelah market RESOLVED (blok resolve + konfirmasi):
scripts/testnet.sh payout <market_id>   # rencana payout + tanda tangan FROST 2-of-3 (member 1 dan 2) + broadcast
scripts/testnet.sh status | logs <nama> | down
```

Biaya TAZ sangat kecil: pemenang menerima kembali pool dikurangi fee 1%, jadi dua bet minimum menghabiskan sekitar 0,0002 TAZ ditambah biaya jaringan.

State ada di `.testnet/run` (diabaikan git). `down` lalu `up` melanjutkan market yang sudah ada. **Jangan** membawa market lama berjam-jam: indexer harus memindai ulang semua block sejak market itu lahir lewat RPC yang dibatasi per detik (sekitar 0,3 block per detik). Pakai `fresh` untuk memulai market baru dari tip.

**Bukti on-chain di UI:** halaman market memiliki panel "On-chain proof" (txid pengumuman, txid payout, tinggi block, tautan ke explorer lewat `NEXT_PUBLIC_EXPLORER_URL`); halaman Verify memuat txid yang sama, alamat escrow dan viewing key. Txid **bet** sengaja tidak dipublikasikan (privasi); bet terlihat sebagai tinggi block di feed anonim.

**Terbukti di testnet NU7 (market `39e7234e…`, 2026-10-25):** pengumuman (tx `497294c4…`, block 4.536.945), dua bet COUNTED (pool 2.000.000 zat), resolusi chain-verified (YES, block 4.536.977), payout FROST 2-of-3 (tx `afa18033…`, CONFIRMED di block 4.537.013), market SETTLED, dan `frosted-verify all` PASS di 12 pemeriksaan terhadap node publik.

**Catatan:**
- Blok testnet NU7 seharusnya 25 detik, tetapi terukur sekitar satu menit dan kadang berhenti beberapa menit; hitung jendela market dengan longgar.
- Dana yang baru diterima baru bisa dibelanjakan setelah beberapa konfirmasi, jadi `announce` gagal berkali-kali di awal. Itu normal.
- Log bisa memuat URL RPC; gunakan `scripts/testnet.sh logs`, yang menyamarkannya.
- Editor markdown (mis. Orca) bisa menulis ulang file ini dan merusaknya; tutup file itu di editor sebelum menjalankan skrip dokumentasi.
