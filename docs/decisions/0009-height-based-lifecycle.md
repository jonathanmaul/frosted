# ADR-0009: Siklus hidup market berbasis tinggi blok dan announcement terautentikasi

- Status: Accepted
- Tanggal: 2026-10-21
- Bagian spec: US-02, US-08, UF-1, UF-2, §19.3, §19.5, SD-1, SD-3, §23.1, §23.3 J-4, §23.4, M4.3, M4.4, M7.4, §36 V-12

## Context

- Bet yang masuk saat `ANNOUNCING` (antara `birthday_height` dan announcement terindeks) tidak punya aturan.
- §19.5 menerima announcement "yang dikirim oleh coordinator", padahal pengirim transaksi shielded tidak bisa diketahui.
- Jika announcement tidak pernah terkonfirmasi, market tertahan di `ANNOUNCING`.
- State machine menutup market saat `tip > close_height`, sementara UI menutup saat `tip ≥ close_height`.
- Jalur VOID berbeda antara §23.1 (`RESOLVED → VOID`) dan SD-3 (langsung VOID).

## Decision

- Penghitungan bet hanya bergantung pada tinggi blok: bet dihitung jika `birthday_height ≤ h ≤ close_height`, termasuk saat `ANNOUNCING`. UI tetap menonaktifkan Bet Builder sampai `OPEN`.
- Announcement sah = memo `FRSTM1` ke outcome 0, bernilai `MIN_ANNOUNCE_ZAT` (> `SWEEP_THRESHOLD_ZAT`), dengan `rules_hash` yang sama dengan hash `market_definition`. Hanya yang pertama dipakai; FRSTM1 dengan hash lain → `INVALID_MEMO`.
- `ANNOUNCE_TIMEOUT_BLOCKS = 100` `[ASSUMPTION]`: tanpa announcement sah dalam jendela ini, market `VOID` dan semua dana direfund.
- Urutan aturan §19.5: dekripsi → `UNECONOMIC_DUST` → `UNKNOWN_OUTCOME` → announcement → `REJECTED_TRANSPARENT` → `LATE` → `IGNORED_DUST` → `INVALID_MEMO` → `PENDING`.
- Invariant startup: `MIN_ANNOUNCE_ZAT > SWEEP_THRESHOLD_ZAT` dan `MIN_BET_ZAT > SWEEP_THRESHOLD_ZAT`.
- Market `CLOSED` saat `tip ≥ close_height`.
- `RESOLVED → VOID` jika `winning_total = 0` (§23.1 dipertahankan, SD-3 disesuaikan).

## Consequences

- Verifier bisa menghitung ulang semua status tanpa mengetahui status market di server.
- FRSTM1 palsu dari pihak lain tidak bisa membuka market atau mengubah aturan (T-13).
- Bet transparent ≤ `SWEEP_THRESHOLD_ZAT` berstatus `UNECONOMIC_DUST`, bukan `REJECTED_TRANSPARENT`; refund-nya memang tidak ekonomis.
- V-12 (announcement bernilai 0) tidak relevan lagi.
