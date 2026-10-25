# DESIGN.md — Brief visual Frosted

Brief tetap untuk semua UI `frosted-web`. Setiap change UI **wajib** merujuk dokumen ini dan memakai token serta komponen dasarnya. Perubahan aturan di sini lewat ADR, bukan diam-diam di kode.

Sumber layar dan perilaku: `docs/PROJECT_SPEC_FROSTED.md` §16 (screen list), §17 (wireframe), §13 (NFR privasi, UX, aksesibilitas).

## 1. Arah visual

**"Frozen ledger"**: dokumen teknis yang bisa diaudit, bukan dashboard crypto. Kesan yang dituju: angka yang tenang dan bisa dicek, pengirim yang tetap buram.

## 2. Palet (CSS variables, "Paper" terang; lihat §14 dan ADR-0019)

> Tabel di bawah adalah palet gelap lama. Nilai yang berlaku ada di §14.

| Token | Nilai | Pemakaian |
| --- | --- | --- |
| `--bg` | `#070B12` | Latar halaman |
| `--surface` | `rgba(255,255,255,0.04)` | Panel datar |
| `--border` | `rgba(180,210,255,0.14)` | Garis 1px, grid hairline |
| `--ice` | `#DCEBFF` | Teks utama |
| `--ice-muted` | `#8FA8C8` | Teks sekunder, label |
| `--accent` | `#F4B728` | **Hanya** CTA utama |
| `--yes` | `#7FD1B9` | Outcome YES, selalu dengan label dan persen |
| `--no` | `#E58C8A` | Outcome NO, selalu dengan label dan persen |

- Dark-only: tidak ada tema terang.
- Warna outcome tidak pernah berdiri sendiri; selalu disertai label dan persentase (NFR-A2).
- Teks di atas `--accent` memakai `--bg`.

## 3. Tipografi (self-host, semua OFL, tanpa CDN)

| Peran | Font | Aturan |
| --- | --- | --- |
| Display / judul | Instrument Serif | `clamp(48px, …, 96px)`, tracking sedikit rapat |
| Body / UI | IBM Plex Sans 400 / 500 | — |
| Semua angka (blok, ZEC, odds, hash, persen) | IBM Plex Mono | `font-variant-numeric: tabular-nums` |

- Font di-bundle dari paket npm `@fontsource/*` dan disajikan dari origin sendiri (NFR-P2).
- Skala tipe tetap **5 tingkat** (token `--type-1` … `--type-5`); ukuran lain tidak dipakai.

## 4. Layout

- Grid 12 kolom, rata kiri, asimetris. Garis grid hairline 1px (`--border`) boleh terlihat di landing.
- Section bernomor `01 / 02 / 03` dalam mono kecil.
- Aturan awal hero: teks di kolom 1–6, rata bawah; objek 3D di kolom 7–12. **Bukan** hero di tengah. **Digantikan khusus pembuka landing oleh permintaan pengguna berikutnya (ADR-0017): tulisan di tengah layar, langsung masuk Through the Frost saat scroll.** Aturan layout halaman lain tetap berlaku.
- Header: logo teks `FROSTED` + ticker tinggi blok (mono) yang berdetak saat blok baru.

## 5. Permukaan

- Kaca (blur + transmission) **hanya** untuk objek 3D dan panel Bet Builder.
- Semua komponen lain datar: pemisah garis 1px, radius 2–4px, tanpa drop shadow.

## 6. Hero 3D landing (S-01 bagian atas)

- Prosedural, tanpa file model eksternal. Permintaan pengguna berikutnya mengizinkan `.glb` lokal: generator milik proyek menghasilkan `web/public/models/frost-ledger.glb`; tidak memakai model unduhan atau layanan aset eksternal.
- Prisma/kubus kaca beku dengan `MeshTransmissionMaterial` dari `@react-three/drei` (terverifikasi ada di drei 10.7.9: props `transmission`, `thickness`, `roughness`, `chromaticAberration`, `samples`, `resolution`, `backside`), berputar pelan.
- Di dalamnya: bar odds YES/NO market unggulan dari `GET /v1/markets`, bergerak saat SSE `odds_updated`.
- Di belakang kaca: 5–8 siluet kapsul buram yang bergerak pelan (penaruh anonim).
- Kursor mengurangi frost secara lokal, hanya memperjelas angka, **tidak pernah** siluet.
- Tagline: "Public odds. Invisible bettors." + CTA "Browse markets" (accent) + "How it works" (ghost).

## 7. Animasi dan library

| Kebutuhan | Library | Catatan |
| --- | --- | --- |
| 3D | `@react-three/fiber` 9.x + `@react-three/drei` 10.x (`three` 0.18x) | Fiber 9 butuh React 19 < 19.4 |
| Scroll | GSAP + ScrollTrigger + Lenis | Hanya untuk section "02 / How it works" |
| Teks/angka | React Bits (salin source ke `web/components/bits/`) | Lihat di bawah |
| Dialog, tabs, tooltip | Radix UI primitives (unstyled) | Diberi gaya sesuai brief ini |

- Scrollytelling "02 / How it works", 4 langkah: bet masuk → odds terlihat → pengirim tetap buram → payout; memakai objek 3D hero yang sama.
- React Bits yang dipakai (nama terverifikasi di repo `DavidHDev/react-bits`):
  - `DecryptedText` (TextAnimations): angka odds/pool terdekripsi; baris "bettor" selalu teracak dan tidak pernah terbuka.
  - `CountUp` (TextAnimations) / `Counter` (Components): total pool dan ticker tinggi blok.
  - `Noise` (Animations): tekstur grain latar, opacity ≤ 4%.
  - Semua diubah agar memakai font, warna, dan timing brief ini.
  - Lisensi: MIT + Commons Clause. Boleh dipakai di aplikasi, **tidak boleh** dijual atau diredistribusikan sebagai komponen. File salinan menyimpan notice aslinya dan dikecualikan dari lisensi MIT/Apache proyek (dicatat di README).
- Dependensi React Bits: `motion` (dipin ke versi yang lolos kebijakan umur rilis minimum pnpm; saat ini 14.3.1). Jangan menambah `minimumReleaseAgeExclude`.
- Salinan React Bits di `web/components/bits/` tidak diedit langsung (penyesuaian lewat wrapper di `web/components/ui/`) dan dikecualikan dari ESLint karena kode vendor.
- Gerak hanya untuk event nyata (blok baru, bet terhitung, odds berubah) dan scrollytelling.

## 8. Copy

- Kalimat pendek berbasis angka. Contoh: "12.40 ZEC in the pool. 9 bettors. 0 names."
- Bahasa UI: Inggris (NFR-U3).
- Kata terlarang: revolutionary, seamless, next-gen, unlock, empower, cutting-edge.

## 9. DILARANG (anti "AI generated look")

- Font Inter, Geist, Space Grotesk, atau `system-ui` sebagai font utama.
- Hero terpusat + grid 3 kartu fitur + CTA generik.
- Gradient blob, glow ungu/biru, glassmorphism di kartu biasa.
- Ikon emoji, ikon dekoratif di setiap judul, ilustrasi stok.
- Radius > 4px, kecuali objek 3D.
- React Bits `Aurora`, `Particles`, `Galaxy`, `Silk`, dan `SplitText` di hero.
- shadcn/ui tanpa restyle; video background.

## 10. Performa dan aksesibilitas

- Hero 3D di-lazy load (`next/dynamic`, `ssr: false`) dengan placeholder gambar statis.
- Fallback statis jika WebGL tidak tersedia, perangkat lemah, atau `prefers-reduced-motion`.
- Semua animasi mati saat `prefers-reduced-motion: reduce`.
- LCP landing tidak boleh bergantung pada 3D; Lighthouse accessibility ≥ 90.
- Kontras WCAG 2.1 AA (NFR-A1), odds tidak hanya lewat warna (NFR-A2), semua kontrol bisa dioperasikan keyboard (NFR-A3).

## 11. Komponen dasar

`GlassCard` (hanya untuk Bet Builder), `OddsBar`, `StatusBadge`, `MonoNumber`, `Button` (`primary` = accent, `ghost`), plus layout dan navigasi. Halaman lain memakai komponen ini, bukan membuat versi sendiri.

## 12. Implementasi landing dan rujukan tetap

- Setiap perubahan UI berikutnya wajib membaca dan merujuk `docs/DESIGN.md`; aturan ini juga dicantumkan di `web/AGENTS.md`.
- Komposisi landing terbaru: tulisan pembuka di tengah viewport, satu patung es di belakangnya, wordmark dan ticker minimal di atas. Scroll langsung memasuki Through the Frost. Tiap bab muncul bergantian dalam scene yang sama, tanpa hero pemasaran dua kolom (ADR-0017).
- Mobile: pembuka → perjalanan yang lebih singkat → market. Fallback menampilkan pembuka lalu semua langkah. Penjelasan tidak membutuhkan scroll animation atau WebGL agar dapat dibaca.
- Model dibuat dari geometri Three.js, pencahayaan studio prosedural, dan material transmission; tanpa model, HDRI, font, atau tekstur remote. Geometri patung disimpan dalam GLB lokal hasil generator. Fallback WebP lokal hasil render model yang sama menyimpan bentuk spesimen tanpa mengarang odds.
- Frost kursor berupa patch lokal pada roughness shader, bukan membuka seluruh kaca. Siluet memakai blur yang dibakar ke tekstur, sehingga tidak pernah menjadi tajam.
- Pengecualian gerak ambient hanya rotasi lambat spesimen dan gerak lambat siluet sesuai §6. Grain statis; baris bettor hanya teracak singkat saat event odds/pool. Tidak ada animasi dekoratif berulang lainnya.
- Rendering 3D berhenti ketika spesimen di luar viewport. Perubahan preferensi reduced-motion langsung kembali ke gambar statis. Error WebGL atau context loss juga kembali ke fallback.
- Font Latin dibundle, termasuk italic Instrument Serif. Skala: display responsif 48–96px, judul bagian 48px, UI besar 20px, body 16px, mono kecil 13px. Tidak menambah tingkat ukuran baru.
- Nilai yang tidak tersedia ditampilkan sebagai `—`; tidak membuat angka pasar atau ticker palsu. Daftar lengkap/filter market tetap untuk change layar bettor berikutnya; landing menampilkan satu market unggulan dari API.
- Navigasi halaman biasa mencakup Markets, Leaderboard, My Bets, Persona, How it works. Pembuka immersive hanya menampilkan wordmark, ticker, dan skip ke market; tautan mekanisme tersedia setelah masuk. Skip link, focus keyboard, dan footer trust/privacy tetap tersedia.
- S-12 menyertakan diagram, publik/privat, batasan trust, semua 10 item metadata §24.3, dan FAQ legal. Klaim visual tidak menggantikan disclosure coordinator, komite demo, atau Thaw.

Rujukan API yang diverifikasi: [drei MeshTransmissionMaterial](https://drei.docs.pmnd.rs/shaders/mesh-transmission-material), deklarasi paket terpasang `@react-three/drei/core/MeshTransmissionMaterial.d.ts`, [React Bits DecryptedText](https://reactbits.dev/text-animations/decrypted-text), [CountUp](https://reactbits.dev/text-animations/count-up), [Counter](https://reactbits.dev/components/counter), [Noise](https://reactbits.dev/animations/noise). API salinan lokal diperiksa sebelum dipakai melalui wrapper.

## 13. Through the Frost (ADR-0017)

Perubahan arah ini diminta pengguna: scroll terasa memasuki ruang ledger melalui kaca. Pengguna kemudian memperjelas bahwa pembuka harus berupa tulisan di tengah layar, bukan hero pemasaran. Permintaan terbaru ini menggantikan komposisi hero awal.

- Perjalanan dimulai dari pembuka di tengah layar; Canvas sudah memenuhi viewport sejak awal. Tidak ada mount ulang atau pengunduhan aset model baru.
- Urutan kamera: mendekati kaca → melewati frost → lembar ledger dan bar odds → siluet tetap buram → tiga elemen payout merapat. Gerak kamera dan perpindahan bab sepenuhnya mengikuti scroll dan dapat dibalik.
- Pin desktop berlangsung 4,4 tinggi viewport; versi compact 3. Layar dengan tinggi di bawah 600px, reduced-motion, perangkat lemah, dan kegagalan WebGL menampilkan narasi normal tanpa pin.
- Komposisi bab terbaru mengikuti masukan pengguna: scene penuh tanpa panel kiri atau pemisah vertikal. Kamera mengikuti sumbu tengah dengan parallax kecil; subjek utama berada di area atas, caption rata tengah di bawah. Scrim gelap vertikal yang halus melindungi kontras caption; ini bukan glow atau gradient blob.
- Empat bab tetap persis bet masuk / odds terlihat / pengirim buram / payout. Copy memakai crossfade pendek dan perpindahan vertikal kecil berdasarkan progress, tanpa animasi loop.
- Tersedia tombol bab yang bisa dipakai keyboard, status bab aktif, indikator progress hairline, dan tautan “Skip to markets”. Bab yang tidak terlihat diberi `inert` dan `aria-hidden`.
- Ruang di dalam kaca memakai lembar tipis prosedural, garis grid, dan teks mono. Tidak menambah partikel, glow, musik, video, blob, atau warna baru. Siluet tetap tekstur blur permanen.
- DPR maksimum 1,25 dan environment lokal sekali render. Posisi kamera disimpan di ref dan diperbarui di render loop, bukan React state setiap frame.
- Pin dibersihkan saat viewport atau preferensi gerak berubah. Fallback tidak menyembunyikan satu pun langkah. Navigasi anchor tetap bekerja setelah lazy enhancement menambah ruang pin.
- Saat keluar dari pin, Lenis tetap memproses input dengan `smoothWheel: false`, sehingga momentum lama dibatalkan sebelum browser menggulir secara native. Jangan mem-bypass handler lewat `virtualScroll: () => false`: animasi lama dapat menarik halaman ke belakang. Smoothing aktif kembali saat masuk ke perjalanan.

API scroll diperiksa di deklarasi GSAP terpasang dan [ScrollTrigger](https://gsap.com/docs/v3/Plugins/ScrollTrigger/); pembaruan kamera mengikuti [React Three Fiber hooks](https://r3f.docs.pmnd.rs/api/hooks).

### Patung GLB lokal (dihapus oleh ADR-0019)

> Bagian ini historis: GLB, generator, dan poster lama sudah dihapus; scene sekarang sepenuhnya prosedural (§14).

`web/public/models/frost-ledger.glb` (sekitar 61 KB sebelum kompresi transport) dibuat dengan `node scripts/generate-frost-model.mjs` dari direktori `web`. Geometri original mencakup badan es ber-bevel dengan outline asimetris, facet rim, tiga folio tersuspensi, goresan kalibrasi, dan fissure tipis. Tidak ada identitas, angka pasar contoh, atau tekstur eksternal di aset.

Material runtime menambahkan mikro-frost prosedural, transmission, clearcoat, dan pencahayaan studio lokal. Mesh `IceShell` adalah titik penggantian material; odds dinamis tetap geometri runtime dengan label dan persentase. Generator meng-assert header GLB 2.0, panjang file, mesh utama, ketiadaan buffer eksternal, dan batas 500 KB.

## 14. Paper (ADR-0019)

Menggantikan palet gelap §2 dan aturan permukaan §5/§9 untuk kartu, bayangan, dan gradient tint. Tipografi, skala tipe, copy, privasi, dan aksesibilitas tidak berubah.

| Token | Nilai | Pemakaian |
| --- | --- | --- |
| `--bg` | `#EEF3F8` | Latar halaman (dengan tint radial biru/oranye sangat halus) |
| `--ice` | `#0A1424` | Teks utama (nama token dipertahankan) |
| `--ice-muted` | `#4B6078` | Teks sekunder |
| `--accent` | `#C93509` | CTA utama; teks di atasnya putih |
| `--core` | `#2B59FF` | Indeks section, inti kristal 3D, hover kartu |
| `--panel` | `#FFFFFF` | Kartu dan panel |
| `--band` | `#0A1424` | Satu band gelap per halaman; token di dalamnya di-override |
| `--yes` / `--no` | `#0B7A55` / `#C41F37` | Outcome, selalu dengan label dan persen |

- Kartu (market row, market unggulan, Bet Builder): latar putih, radius `--radius-card` 16px, bayangan lembut, hover naik 3px. Kontrol, tabel, dan badge tetap datar dengan radius 2–4px.
- Setiap halaman punya scene 3D kecil di header (`Scene3D` / `PageHead`: `crystal`, `towers`, `chain`). Hanya dekoratif; hilang pada reduced-motion, perangkat lemah, atau tanpa WebGL.
- Landing: perjalanan Through the Frost memakai kristal, serpihan, salju, menara odds YES/NO, dan dunia ledger putih. Band gelap penutup berisi prinsip Frosted dan kristal.
- Aset 3D prosedural; GLB lama dihapus.
