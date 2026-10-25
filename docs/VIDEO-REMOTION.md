# Membuat video demo Frosted dengan Remotion dan Claude Code

Panduan produksi dari nol sampai file MP4 siap submit. Pasangannya: `docs/DEMO-E2E.md` (cara menjalankan
demo di testnet). Dokumen ini berisi: alur cerita, naskah narasi, daftar rekaman, setup Remotion, prompt
Claude Code per adegan, render, dan daftar cek kualitas.

Jujur soal hasil: tidak ada yang otomatis "pasti bagus". Yang menentukan adalah (1) rekaman layar yang bersih,
(2) narasi yang jelas, dan (3) beberapa putaran revisi di Remotion Studio. Panduan ini memberi struktur dan
prompt yang sudah disesuaikan dengan Frosted, sehingga putaran revisinya sedikit.

## 0. Ringkasan langkah

1. Batas video 2 menit; alur di §1 sudah disesuaikan. Cek juga aturan format dan bahasa.
2. Gladi kotor demo testnet (`docs/DEMO-E2E.md`), lalu hari rekaman: rekam klip sesuai §3.
3. Rekam narasi (§2) atau buat dengan TTS.
4. Siapkan proyek Remotion di luar repo (§4).
5. Pasang brief proyek untuk Claude Code (§5), lalu buat adegan satu per satu dengan prompt §6.
6. Review di Studio, revisi, render (§7), cek kualitas (§8), upload.

Perkiraan waktu: rekaman 1,5 jam (termasuk menunggu block), narasi 30 menit, Remotion 3–5 jam.

## 1. Alur cerita (maksimum 2:00)

Batas hackathon 2 menit. Seluruh siklus tetap ditampilkan, tapi waktu tunggu block dipotong dengan "lompatan
waktu" yang diberi label jelas. Sekitar sepertiga awal berupa animasi (problem dan solusi); sisanya rekaman
demo asli; penutup animasi. Target total 1:55 supaya ada sisa aman.

| Bagian | Waktu | Jenis | Isi |
| --- | --- | --- | --- |
| Problem | 0:00–0:20 | Animasi | Di prediction market publik, setiap bet terikat ke dompet: siapa bertaruh apa terlihat semua orang, bisa dilacak, ditiru, ditekan. |
| Solusi | 0:20–0:35 | Animasi | Frosted: bet = transaksi shielded Zcash ke escrow per outcome; viewing key publik menghitung pool; pengirim tetap tersembunyi; payout FROST 2-of-3. |
| Market | 0:35–0:43 | Rekaman | Operator membuat market (aturan dikunci on-chain), halaman market OPEN. |
| Bet | 0:43–1:05 | Rekaman | Bet Builder di browser → QR/URI → dibayar dari dompet → pool naik live, feed anonim. |
| Resolusi | 1:05–1:12 | Animasi + rekaman | Penghitung block cepat "≈ 30 blocks later" → RESOLVED dari data chain. |
| Payout | 1:12–1:25 | Animasi + rekaman | 2 dari 3 anggota menyetujui plan hash → satu transaksi payout. |
| Bukti | 1:25–1:45 | Rekaman | Txid pengumuman dan payout di explorer publik; Verify. |
| Penutup | 1:45–1:55 | Animasi | Batasan demo (testnet, komite dipegang satu orang), logo, tagline, link repo. |

Yang dipangkas dari versi panjang: My Bets dan halaman SETTLED hanya muncul sekilas (2–3 detik) atau dilewati;
terminal hanya muncul sependek mungkin.

## 2. Naskah narasi (bahasa Inggris, sekitar 250 kata, kira-kira 1:50)

Bicara jelas, sekitar 140 kata per menit. Rekam per paragraf, satu file per paragraf (`vo-01.wav` dst), supaya
mudah dipasang dan diulang. Pilihan: suara sendiri (mikrofon headset, ruangan tenang) atau TTS (misalnya
ElevenLabs; cek lisensinya).

1. **Problem (0:00–0:20).** "Prediction markets are public by default. Every bet is tied to a wallet, so anyone
   can see who bet what, follow their history, copy them, or pressure them. The odds should be public. The
   people shouldn't."
2. **Solution (0:20–0:35).** "Frosted is a prediction market on Zcash. Each bet is a shielded payment to the
   market's escrow. A public viewing key lets anyone count the pools, but nobody can see who sent them."
3. **Market (0:35–0:43).** "The operator opens a market. Its rules are locked on-chain."
4. **Bet (0:43–1:05).** "A bettor picks an outcome and a payout address. The bet is sealed in the browser. No
   account, no wallet connection. Any Zcash wallet pays it, and a block later the pool updates live. The feed
   shows amounts and outcomes, never senders."
5. **Resolve (1:05–1:12).** "When the market closes, the result is read straight from the chain."
6. **Payout (1:12–1:25).** "Two of three committee members approve the payout plan, and a FROST threshold
   signature pays the winners in one transaction."
7. **Proof (1:25–1:45).** "Every step is a real testnet transaction, visible on a public explorer, and anyone can
   recompute the result with their own node."
8. **Close (1:45–1:55).** "This demo runs on testnet with one person holding all three shares. Frosted: public
   odds, invisible bettors."

## 3. Daftar rekaman

Pengaturan: layar 1920×1080 (atau rekam Retina dan ekspor ke 1080p), 30 atau 60 fps, kursor terlihat, notifikasi
macOS dimatikan (Focus mode), bookmark bar dan ekstensi browser disembunyikan, terminal dengan font ≥ 18 pt
dan latar gelap polos. Jangan pernah menampilkan isi `~/.config/frosted/testnet.env`.

Alat rekam: `Cmd+Shift+5` bawaan macOS cukup. Screen Studio (berbayar) memberi zoom otomatis mengikuti kursor;
OBS gratis. Zoom dan sorotan juga bisa dibuat di Remotion, jadi rekaman polos tidak masalah.

Rekam lebih panjang dari yang dibutuhkan (2–3 detik diam di awal dan akhir setiap klip). Nama file:

| File | Isi | Mulai | Selesai |
| --- | --- | --- | --- |
| `01-landing.mp4` | Landing, scroll pelan melewati Through the Frost sampai daftar market | Halaman diam 2 s | Kartu market terlihat |
| `02-market-create.mp4` | Terminal: `scripts/testnet.sh market 30 "..."` | Sebelum mengetik | market_id tercetak |
| `03-market-open.mp4` | Halaman market OPEN, label LIVE, menara odds | Halaman dibuka | 5 s diam |
| `04-bet-builder.mp4` | 4 langkah Bet Builder sampai QR dan Copy URI | Langkah 01 | URI tersalin |
| `05-pay.mp4` | Terminal: `scripts/testnet.sh pay '...'` | Sebelum menempel | txid tercetak |
| `06-pool-live.mp4` | Halaman market saat pool naik dan baris feed muncul | Sebelum berubah | Setelah berubah |
| `07-my-bets.mp4` | My Bets dengan status COUNTED | Halaman dibuka | 4 s diam |
| `08-resolved.mp4` | Halaman market berstatus RESOLVED, bagian Resolution & payout | Halaman dibuka | 4 s diam |
| `09-payout.mp4` | Terminal: `scripts/testnet.sh payout <id>` | Sebelum Enter | txid payout tercetak |
| `10-proof.mp4` | Panel On-chain proof → klik View on explorer → halaman zecblock | Panel terlihat | Halaman tx explorer |
| `11-verify.mp4` | Halaman Verify, lalu (opsional) terminal `frosted-verify all` PASS | Halaman dibuka | Hasil PASS |
| `12-settled.mp4` | Halaman market SETTLED | Halaman dibuka | 3 s diam |

Tip: klip 06 butuh menunggu sekitar 1 menit; rekam terus, nanti dipercepat di Remotion.

## 4. Setup proyek Remotion

Di luar repo Frosted (lihat alasannya di percakapan: klip besar, repo submission tetap bersih):

```bash
cd ~/Workspace/Web3/Hackathon/jonathanmaul
npx create-video --yes --blank frosted-video
cd frosted-video
npx remotion skills add            # Remotion Agent Skills untuk Claude Code
npm i @remotion/google-fonts       # Instrument Serif, IBM Plex Sans, IBM Plex Mono
mkdir -p public/clips public/vo public/brand
cp ../frosted/web/public/logo/*.png public/brand/
# salin klip dan narasi:
#   public/clips/01-landing.mp4 ... 12-settled.mp4
#   public/vo/vo-01.wav ... vo-08.wav
```

Jalankan preview dan Claude Code di dua terminal:

```bash
npm run dev      # Remotion Studio di browser
claude           # terminal kedua, di folder frosted-video
```

Lisensi Remotion: cek <https://www.remotion.dev/docs/license> (sumber pihak ketiga menyebut gratis untuk individu
dan tim kecil; belum diverifikasi dari halaman resmi).

## 5. Brief proyek untuk Claude Code

Buat file `frosted-video/CLAUDE.md` berisi teks berikut. Claude Code membacanya otomatis di setiap sesi, jadi
gaya visual tetap konsisten tanpa diulang di setiap prompt.

```markdown
# Frosted demo video

A 1920x1080, 30 fps hackathon demo video, HARD LIMIT 2:00 (target 1:55), for Frosted, an anonymous prediction market on
Zcash ("Public odds. Invisible bettors."). Built with Remotion; use the installed Remotion skills.

## Visual system (match the product)
- Background #EEF3F8 (paper), ink text #0A1424, muted #4B6078, core blue #2B59FF, CTA orange #C93509,
  bright orange accent #FF4F1F, YES #0B7A55, NO #C41F37, dark band #0A1424 with text #EAF2FF.
- Fonts via @remotion/google-fonts: Instrument Serif (titles, italic for emphasis), IBM Plex Sans (body),
  IBM Plex Mono (numbers, txids, block heights, labels in small caps with letter spacing).
- Shapes: faceted crystal / icosahedron, small orange tetrahedron shards, thin 1px hairlines, white cards with
  16px radius and soft shadow. No neon glow, no purple gradients, no emoji.
- Logo: public/brand/frosted-logo.png (wordmark) and public/brand/frosted-mark.png (icon).

## Motion
- Calm and precise: spring() with high damping, 12-20 frame entrances, no bouncing text.
- Screen recordings sit in a white rounded "window" card with a soft shadow on the paper background, slightly
  scaled (0.86) so captions fit. Use OffthreadVideo, muted. Trim each clip in its Sequence; speed up waiting
  parts with playbackRate.
- Captions: lower-left, small mono label above (e.g. "02 / BET"), short Instrument Serif line below.
- Highlights: a 2px core-blue rounded rectangle that draws on around the area being explained, plus an
  optional 1.15x zoom into that area.

## Structure
- src/Root.tsx registers one composition "FrostedDemo" (1920x1080, 30 fps) and each scene as its own
  composition for previewing.
- One file per scene in src/scenes/, composed in src/FrostedDemo.tsx with TransitionSeries (fade, 12 frames).
- Narration: public/vo/vo-01.wav ... vo-08.wav, one per scene group; scene length follows its narration
  length (use getAudioDurationInSeconds or a fixed duration table in src/timing.ts).
- Keep all text in one file src/copy.ts so wording can be changed in one place.

## Facts (do not invent others)
- Bets are shielded Zcash transactions to a per-outcome escrow address; the viewing key is public.
- Market rules are locked on-chain by the announcement memo (rules hash).
- Resolution of this market is read from chain data (no human vote).
- Payout: FROST 2-of-3 threshold signature. In this demo one person holds all three shares (trusted dealer).
- Bet transaction ids are never shown; announcement and payout txids are public.
- Network: Zcash testnet (NU7). Explorer: testnet.zecblock.com. Nothing on mainnet.
```

## 6. Prompt Claude Code, per adegan

Jalankan berurutan. Setelah setiap prompt, cek di Studio, lalu minta revisi spesifik (lihat §6.10).

### 6.1 Kerangka

```text
Set up the project structure from CLAUDE.md: Root.tsx with the FrostedDemo composition and per-scene
compositions, src/copy.ts, src/timing.ts with a duration table (placeholder 5 s per scene), a shared
<Window> component for screen recordings, a <Caption> component and a <Highlight> component.
Use the brand fonts and colors. Render a placeholder card per scene so I can see the timeline in Studio.
```

### 6.2 Problem (0:00–0:20)

```text
Create src/scenes/Problem.tsx, 20 s, with public/vo/vo-01.wav. Explain the problem with animation only.
Beat 1 (0-6 s): on the dark band background, a public ledger table builds row by row: wallet address, outcome
(YES/NO), amount. Beat 2 (6-12 s): one wallet row gets highlighted; lines connect it to three of its other
bets across the table and a small "profile" card appears ("0x…3f9: 14 bets, 82% YES"), showing that
identities can be tracked and copied. Beat 3 (12-17 s): small labels pop near the highlighted wallet:
"tracked", "copied", "pressured". Beat 4 (17-20 s): all wallet addresses blur and scramble into "░▒▓" while
amounts and outcomes stay sharp; the line "The odds should be public. The people shouldn't." appears.
Calm motion, readable text, brand palette only.
```

### 6.3 Solusi (0:20–0:35)

```text
Create src/scenes/Solution.tsx, 15 s, with vo-02.wav. On paper background: the Frosted logo appears top-left.
A browser card on the left sends a "shielded note" (amount + outcome visible, sender shown as ░▒▓) into a
faceted crystal labelled "Escrow". Above the crystal, "Public viewing key" and YES/NO bars that grow with
percentages in IBM Plex Mono. On the right, three member keys; two light up core blue and one payout note
leaves the crystal: "FROST 2-of-3". Time every step to the narration; keep it to 15 s.
```

### 6.4 Market dibuat

```text
Create src/scenes/Market.tsx, 8 s, using 3 s of clips/02-market-create.mp4 then 5 s of clips/03-market-open.mp4 in <Window>.
Caption "01 / MARKET" + "Rules locked on-chain." Highlight the printed market id in the terminal, then on the
market page highlight the status badge and the LIVE label. Narration vo-03.wav.
```

### 6.5 Bet di browser

```text
Create src/scenes/Bet.tsx (about 13 s of the 22 s bet section) with clips/04-bet-builder.mp4 and clips/05-pay.mp4, narration vo-04.wav.
Captions in sequence: "02 / BET" + "Sealed in the browser.", then "No account. No wallet connect.", then over
the terminal clip "Any Zcash wallet can pay it." Highlight each Bet Builder step as it is clicked, zoom 1.15x
into the QR code for 2 s. Trim hard: show each Bet Builder step for about 2 s and the terminal for about 2 s.
```

### 6.6 Pool naik live

```text
Create src/scenes/Live.tsx (part of the 22 s bet section, about 9 s) with clips/06-pool-live.mp4 (speed up the waiting part to 8x with playbackRate,
show a small "≈ 1 block later" mono label while sped up) and, only if time allows, 2 s of clips/07-my-bets.mp4. It shares narration vo-04.wav with Bet.tsx.
Highlight the pool total when it changes and the new anonymous feed row; caption "Amounts visible.
Senders shielded." If My Bets is included, caption "Receipt stays on this device." and highlight COUNTED.
```

### 6.7 Lompatan waktu + resolusi

```text
Create src/scenes/Resolve.tsx, 7 s, narration vo-05.wav. A full-screen block counter in IBM Plex Mono
counts quickly from the close height to the resolve height (use the numbers I give you: close X, resolve Y),
with a thin progress hairline. Then cut to clips/08-resolved.mp4 with caption "03 / RESOLVE" +
"Read from the chain. No vote." and highlight the RESOLVED status.
```

### 6.8 Payout

```text
Create src/scenes/Payout.tsx, 13 s, with clips/09-payout.mp4, narration vo-06.wav. Before the clip, a 3 s overlay
on paper: the plan hash in mono, three member cards; members 1 and 2 "type" the hash and turn core blue,
member 3 stays grey; the two combine into one signature line. Then the terminal clip; highlight the printed
payout txid. Caption "04 / PAYOUT" + "Two of three sign. One transaction."
```

### 6.9 Bukti dan penutup

```text
Create src/scenes/Proof.tsx, 20 s, with clips/10-proof.mp4 and clips/11-verify.mp4 (skip 12-settled unless there is time),
narration vo-07.wav. Highlight the announcement txid and the payout txid in the On-chain proof panel, follow
the click to the explorer and zoom 1.2x into the transaction page; caption "05 / PROOF" + "Real testnet
transactions." Then the Verify page / PASS output with caption "Recompute everything with your own node."

Create src/scenes/Outro.tsx, 10 s, narration vo-08.wav: first a calm card "Testnet demo · one person holds
all three committee shares" (honest limits), then the dark band with the Frosted wordmark (white version via
CSS filter or a white text fallback), the line "Public odds. Invisible bettors.", and the repo URL
github.com/jonathanmaul/frosted in mono.
```

### 6.10 Menyusun dan merapikan

```text
Assemble all scenes in FrostedDemo.tsx with TransitionSeries fades (12 frames). Set each scene duration from
its narration length plus 0.3 s. The total MUST stay at or under 1:55; if it is longer, report which scenes to
cut and by how much instead of speeding up the narration. Add a very quiet background music track if public/music.mp3 exists
(volume 0.08, fade in/out). Then list the total duration and any scene longer than its narration.
```

Contoh revisi yang efektif (spesifik, satu hal per prompt):

- "In Bet.tsx the caption overlaps the Continue button at 0:06; move captions to the top-left for that scene."
- "The highlight in Proof.tsx lags the cursor by about 10 frames; start it 10 frames earlier."
- "Problem beat 2 is too fast to read; extend it by 1 s and shorten beat 3."
- "Make all entrances 25% slower; it feels rushed."

## 7. Render

```bash
npx remotion render FrostedDemo out/frosted-demo.mp4 --codec=h264 --crf=18
```

Preview satu adegan cepat: `npx remotion render Bet out/bet.mp4`. Thumbnail:
`npx remotion still FrostedDemo out/thumb.png --frame=200`.

## 8. Daftar cek kualitas sebelum submit

- [ ] Durasi ≤ 2:00 (target 1:55); 35 detik pertama menjelaskan problem dan solusi.
- [ ] Narasi jelas, tidak tertutup musik; tidak ada jeda hening panjang.
- [ ] Semua teks terbaca di layar laptop kecil (caption ≥ 32 px pada 1080p).
- [ ] Tidak ada rahasia di layar: URL RPC Chainstack, isi `testnet.env`, file kunci di `.testnet/private`.
- [ ] Tidak ada txid bet ditampilkan sebagai bagian dari Frosted (txid di terminal `pay` boleh dipotong atau diblur).
- [ ] Txid pengumuman dan payout di video sama dengan yang di explorer.
- [ ] Batasan demo disebutkan (komite satu orang, testnet).
- [ ] Logo dan link repo di akhir; repo sudah publik.
- [ ] Ditonton ulang penuh sekali tanpa suara (caption cukup menjelaskan) dan sekali dengan suara.
- [ ] Upload (misalnya YouTube unlisted) dan cek link bisa dibuka dari jendela private.
