# ADR-0002: Thaw Tag v2 terikat ke data bet, tanpa `thaw_seq` dan gap limit

- Status: Accepted
- Tanggal: 2026-10-21
- Bagian spec: §0.2, US-04, US-12, US-13, UF-1, UF-5, §19.3, SD-5, §21.2, §22.3, §22.4, §23.4, M2.4, M9.2, M9.3, T-19

## Context

Tag v1 adalah `BLAKE2b(K_root, market_id ‖ thaw_seq)`, dan Thaw memindai `thaw_seq = 0..` sampai `THAW_GAP_LIMIT` tag berturut-turut tidak ditemukan. Ada tiga masalah:

1. **Cherry-picking tetap mungkin.** Forecaster bisa menaruh bet "hedge" dengan `seq` jauh di atas frontier. Kalau menang, ia "menjembatani" gap dengan bet kecil di market berikutnya; kalau kalah, bet itu tidak pernah ditemukan. Ini merusak klaim inti P-4.
2. **Inkonsisten antar bagian.** `thaw_seq` global per Persona (§21.2), tapi SD-5 memindai per market, sehingga bet jujur terlewat begitu ada >20 bet di market lain.
3. **Penyalinan tag.** Memo terbaca siapa pun lewat UIVK; payload bisa disalin ke outcome lain di market yang sama untuk mengotori reputasi.

## Decision

```
thaw_tag = BLAKE2b-256(key = K_root, personal = "FrostedThawTag02",
                       data = market_id[16] || outcome_index u8
                              || amount_zat u64 LE || env_epk[32])
```

- `thaw_seq`, `next_thaw_seq`, dan `THAW_GAP_LIMIT` dihapus.
- Thaw menghitung ulang tag untuk **setiap** bet ber-tag dari data publiknya dan membandingkan dengan `thaw_tag` di memo.
- Tag kembar: hanya kemunculan pertama (urut tinggi blok, posisi tx, indeks action) yang dihitung; sisanya `DUPLICATE_TAG`, tidak dihitung di reputasi, payout tetap normal.

## Consequences

- Thaw berbiaya O(jumlah bet ber-tag); tidak ada keputusan yang bisa diambil setelah hasil diketahui.
- Menyalin tag ke bet dengan data berbeda tidak cocok. Salinan persis tetap membayar ke receiver Persona, dan hanya kemunculan pertama yang dihitung.
- Tag mengikat jumlah. Jika wallet mengizinkan jumlah diedit sebelum kirim, bet itu tidak terkait ke Persona. My Bets menandai `AMOUNT_MISMATCH`; M0.6 mencatat perilaku Zodl (V-25).
