# ADR-0005: Hash kanonik dengan JCS, `market_definition` lengkap, `note_ref`

- Status: Accepted
- Tanggal: 2026-10-21
- Bagian spec: §0.2, US-01, US-16, UF-6, §19.3, SD-2, SD-4, SD-6, §21, §22.1, §22.3, §22.4, §23.4, §25, M2.8

## Context

- `rules_hash` memakai `canonical_json(market_definition)` tanpa definisi field maupun skema kanonikalisasi.
- Isi yang di-hash untuk `plan_hash` tidak didefinisikan. US-16 dan S-10 mengklaim verifier bisa mencocokkannya, sementara SD-6 hanya membandingkan ringkasan.
- Item plan diurutkan menurut `bet_fingerprint`, padahal note yang disapu tidak punya fingerprint dan payload yang disalin menghasilkan fingerprint kembar.
- `/v1/envelope-key` mengembalikan `key_id` yang tidak punya tempat di payload FRST1.

## Decision

- Kanonikalisasi JSON memakai **RFC 8785 (JCS)**; semua angka < 2^53 (assertion).
- `rules_hash = BLAKE2b-256("FrostedRules0001", JCS(market_definition))`, dengan `market_definition` = `{version, params_version, network, market_id, question, outcomes[{outcome_index, label, address}], resolution_type, resolution_params, birthday_height, close_height, resolve_height, fee_bps, min_bet_zat, refund_fee_zat, min_confirmations, sweep_threshold_zat, announce_timeout_blocks, uivk, envelope_pk, treasury_address}`. Untuk `COMMITTEE`, `resolution_params` memuat `dispute_window_blocks` dan `void_after_blocks`. Verifier tidak boleh butuh tabel eksternal.
- `envelope_pk` masuk `market_definition`, sehingga terkunci lewat announcement. Endpoint `/v1/envelope-key` dan `key_id` dihapus.
- `note_ref = hex(BLAKE2b-256("FrostedNoteRef01", txid ‖ action_index u32 LE)[0..8])`: kunci unik dan urutan item plan. `bet_fingerprint` tetap disimpan.
- `receiver_commitment = hex(BLAKE2b-256("FrostedRcvr00001", orchard_receiver ‖ bet_salt))`.
- `plan_hash = BLAKE2b-256("FrostedPlan00001", JCS(plan))`, dengan `plan = {market_id, plan_kind, plan_cutoff_height, result, operator_fee_zat, items[{note_ref, bet_fingerprint|null, kind, amount_zat, receiver_commitment|null}]}`. Item `SWEEP`/`UNCLAIMABLE` memakai alamat Treasury yang publik.
- `GET /v1/markets/{id}/payout` mengembalikan plan publik lengkap; `PublicBet` memuat `note_ref`.

## Consequences

- Publik bisa mengecek jenis dan jumlah payout per bet beserta totalnya, tetapi tidak bisa melihat alamat. Setiap bettor bisa mengecek item miliknya.
- API menghitung ulang kedua hash saat ingest dan menolak yang tidak cocok (`RULES_HASH_MISMATCH`, `PLAN_HASH_MISMATCH`).
- Butuh crate JCS yang sesuai standar dan bisa dikompilasi ke WASM (V-24).
