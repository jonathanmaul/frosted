# ADR-0003: Sapuan dana tak terhitung, `UNECONOMIC_DUST`, dan `plan_cutoff_height`

- Status: Accepted
- Tanggal: 2026-10-21
- Bagian spec: §0.2, §11 F-05, US-07, US-10, UF-1, §19.1, §19.5, §19.6, §21, §22.3, §23.2, §23.2a, §23.4, §24.2 T-18, §24.3, §27.4, M7.5, M8.2, M8.3, M8.8

## Context

Nasib dana yang tidak terhitung (`INVALID_MEMO`, `IGNORED_DUST`, note announcement, bagian `UNCLAIMABLE`) tidak didefinisikan: disebut "jadi fee", "unclaimable", dan terminal di bagian berbeda, sementara rumus payout tidak menghitungnya. Note ke diversifier yang bukan Outcome Address diberi status `INVALID_MEMO` yang menyesatkan. Selain itu, dana bisa masuk ke Outcome Address kapan saja, termasuk setelah plan dibuat.

Menyapu semua note membuka serangan spam: note 1 zat memaksa operator membayar 5.000 zat fee dan satu sesi FROST per note.

## Decision

- Status baru `UNKNOWN_OUTCOME` untuk note ke diversifier yang bukan Outcome Address.
- Note dengan nilai > `SWEEP_THRESHOLD_ZAT` (5.000 zat) yang tidak terhitung (`IGNORED_DUST`, `INVALID_MEMO`, `UNKNOWN_OUTCOME`, announcement, `UNCLAIMABLE`) dibelanjakan di Payout Batch ke **Treasury**: UA Orchard-only milik operator yang dicantumkan di `market_definition`. Status akhirnya `SWEPT`.
- Note ≤ `SWEEP_THRESHOLD_ZAT` berstatus `UNECONOMIC_DUST`: tidak pernah dibelanjakan, tidak ada sesi FROST.
- Plan memuat `plan_cutoff_height` (= tip saat plan dibuat). Note setelah itu tidak ditangani otomatis; tetap di escrow dan didokumentasikan. Operator bisa membuat plan `FOLLOW_UP` manual (pipeline sama, `plan_hash` sendiri).

## Consequences

- Klaim yang ditulis: saldo escrow setelah `SETTLED` = Σ `UNECONOMIC_DUST` + note yang masuk setelah `plan_cutoff_height`.
- Spam dust tidak menambah beban signing maupun fee.
- Alamat Treasury dan semua sapuan bersifat publik (§24.3).
- Coordinator punya insentif menandai pemenang jujur sebagai `UNCLAIMABLE`; dimitigasi di ADR-0006.
