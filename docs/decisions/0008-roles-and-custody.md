# ADR-0008: Peran operator vs komite, dan kustodian kunci di demo

- Status: Accepted
- Tanggal: 2026-10-21
- Bagian spec: §0.2, §4, §6, SD-1, §24.2 T-1, §30, §31, §32, §33, M12.3, §36 Q-1

## Context

§6 menyebut Market Operator sebagai pembuat market, tapi §22.2 versi 0.1 mewajibkan tanda tangan *member* untuk membuat market. Siapa Committee Member untuk demo juga belum ditentukan (Q-1). Proyek dikerjakan satu developer.

## Decision

- **Operator** membuat market, menulis aturan, dan menjalankan payout lewat `frosted-operator` (ADR-0007).
- **Committee** hanya untuk resolusi Committee-Resolved Market dan tanda tangan payout FROST.
- Pada demo MVP, satu developer memegang kunci coordinator dan ketiga share FROST. Trusted dealer boleh dipakai dengan label jelas.

## Consequences

- Mitigasi T-1 "member dari pihak berbeda" tidak berlaku untuk demo; secara kepercayaan, demo setara kustodian tunggal. Sifat 2-of-3 ditunjukkan secara teknis saja.
- Committee-Resolved Market di demo punya satu pemutus nyata; jendela sengketa bersifat simulasi.
- README, tabel "Real vs simulated" (§33), naskah demo (§31), dan pitch (§32) menyatakan ini secara eksplisit.
