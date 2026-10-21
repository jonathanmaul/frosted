# ADR-0004: Operator menanggung semua fee transaksi, fail-closed saat kurang

- Status: Accepted
- Tanggal: 2026-10-21
- Bagian spec: US-01, US-10, UF-2, UF-3, §19.4, §19.6, §22 (error), §22.3, §23.4, M2.6, M7.3, M8.2

## Context

§19.6 versi 0.1 memakai `tx_fee_share_for_winners` yang tidak didefinisikan, US-10 menyebut refund VOID "dikurangi bagian fee transaksi", dan contoh `PayoutPublic` membebankan seluruh fee ke pemenang. Jika kekurangan fee boleh memotong pemenang, spam dust (ADR-0003) bisa langsung menurunkan payout pemenang.

## Decision

- `payout_i = floor(amount_i × (pool_total − operator_fee) / winning_total)`. `tx_fee_share_for_winners` dihapus.
- Semua fee ZIP 317 dibayar dari `fee_budget = operator_fee + swept_total + Σ REFUND_FEE_ZAT`.
- Refund tetap `amount − REFUND_FEE_ZAT`.
- Market `VOID`: `operator_fee = 0`.
- Validasi saat create market: `MIN_BET_ZAT × fee_bps / 10_000 ≥ MARGINAL_FEE_ZAT` (dengan nilai sekarang, `fee_bps ≥ 50`). Pelanggaran → `INVALID_FEE_BPS`.
- Jika `tx_fee > fee_budget`: plan **tidak dibuat** (`FEE_SHORTFALL`), operator menambal dulu. Pemenang tidak pernah dipotong.

## Consequences

- Setiap bet `COUNTED` membiayai spend-nya sendiri, sehingga `FEE_SHORTFALL` seharusnya tidak terjadi; tetap dicek sebagai assertion + property test (M2.6).
- Contoh di §22.3 dihitung ulang: `distributed_zat` = `pool_total − operator_fee`.
- Konstanta baru `MARGINAL_FEE_ZAT` dan turunan `MIN_FEE_BPS`. Nilai 5.000 zat masih `[TO VERIFY]` (A-7, V-28).
