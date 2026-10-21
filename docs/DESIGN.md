# DESIGN.md — Brief visual Frosted

Brief tetap untuk semua UI `frosted-web`. Setiap change UI **wajib** merujuk dokumen ini dan memakai token serta komponen dasarnya. Perubahan aturan di sini lewat ADR, bukan diam-diam di kode.

Sumber layar dan perilaku: `docs/PROJECT_SPEC_FROSTED.md` §16 (screen list), §17 (wireframe), §13 (NFR privasi, UX, aksesibilitas).

## 1. Arah visual

**"Frozen ledger"**: dokumen teknis yang bisa diaudit, bukan dashboard crypto. Kesan yang dituju: angka yang tenang dan bisa dicek, pengirim yang tetap buram.

## 2. Palet (CSS variables, dark-only)

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
- Hero: teks di kolom 1–6, rata bawah; objek 3D di kolom 7–12. **Bukan** hero di tengah.
- Header: logo teks `FROSTED` + ticker tinggi blok (mono) yang berdetak saat blok baru.

## 5. Permukaan

- Kaca (blur + transmission) **hanya** untuk objek 3D dan panel Bet Builder.
- Semua komponen lain datar: pemisah garis 1px, radius 2–4px, tanpa drop shadow.

## 6. Hero 3D landing (S-01 bagian atas)

- Prosedural, tanpa file model eksternal.
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
