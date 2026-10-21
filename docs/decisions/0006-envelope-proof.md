# ADR-0006: Envelope Proof dan "Check my plan item"

- Status: Accepted
- Tanggal: 2026-10-21
- Bagian spec: §0.2, §0.3 F-12, §4, US-04, US-07, US-17, §13 NFR-P5, §16 S-04/S-10, §19.2, §19.3, §21.1, §21.2, §22.4, §24.2 T-3, §24.3, M2.3, M3.5, M6.7, M10.1

## Context

Setelah ADR-0003, bagian bet `UNCLAIMABLE` masuk Treasury. Coordinator jadi punya insentif menandai pemenang jujur sebagai `UNCLAIMABLE`, dan publik tidak bisa mendeteksinya karena tidak bisa membuka envelope.

## Decision

- Bet Receipt menyimpan `env_esk` (kunci ephemeral envelope, dibuat di browser, tidak pernah dikirim) dan `bet_salt`.
- My Bets punya "Check my plan item": menghitung `receiver_commitment` dari receipt dan mencocokkannya dengan item plan untuk `note_ref` bet itu.
- Jika bet ditandai `UNCLAIMABLE` secara tidak jujur, bettor bisa menerbitkan **Envelope Proof** (`note_ref`, `env_esk`, receiver, `bet_salt`). `frosted-verify envelope-proof <file>` memeriksa `X25519 basepoint(env_esk) = env_epk`, menurunkan kunci AEAD dengan `envelope_pk` dari `market_definition`, mendekripsi dengan AAD yang sama, dan memastikan plaintext = receiver ‖ `bet_salt`.
- Prioritas SHOULD, di bawah F-12.

## Consequences

- Label `UNCLAIMABLE` palsu bisa dibuktikan siapa pun, terhadap kunci yang sudah terkunci on-chain (ADR-0005).
- Menerbitkan proof membuka payout receiver bet itu; UI memperingatkan sebelum ekspor.
- Menyimpan `env_esk` tidak menambah risiko: receipt sudah menyimpan payout UA dalam bentuk plaintext.
- Deteksi tetap bergantung pada bettor yang mengecek (sisa risiko T-3: rendah–sedang).
