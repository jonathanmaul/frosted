# ADR-0011: Proses spike M0

- Status: Accepted, **Amended 2026-10-21** (lihat bagian "Amendment")
- Tanggal: 2026-10-21
- Bagian spec: §26 (`spikes/`), §28 (M0), SD-4, §36

## Context

- Kode spike sengaja kotor dan tidak dipakai produksi.
- Nomor folder spike tidak cocok dengan nomor task M0 (M0.1 = `s6`, M0.6 = `s1`).
- V-11 (dukungan Ironwood di Zaino) ditugaskan ke M0.4, tapi DoD M0.4 tidak mencakup sync escrow.
- Biaya FROST per spend (satu sesi per note) belum diukur.

## Decision (asli)

- Spike tinggal di `spikes/` dengan hasil di `docs/SPIKES.md`; bukan change OpenSpec. `setup-dev-environment` tetap menjadi change.
- Folder dinomori ulang: `sN` = M0.N (`s1-regtest-env` … `s6-zodl-memo`).
- DoD M0.4 mencakup sync akun escrow lewat Zaino dengan `zcash_client_backend`, serta pencatatan jumlah sesi FROST, durasi per sesi, kemampuan multi-pesan `frostd`, dan fee ZIP 317 aktual.
- DoD M0.1 mencakup `zcash-devtool` yang bisa mengirim note bermemo ke UA Orchard-only. DoD M0.6 mencakup pencatatan apakah Zodl mengizinkan jumlah dari URI diedit.

## Amendment (2026-10-21)

**Yang berubah:** spike M0.2–M0.6 **dijalankan sebagai change OpenSpec** `run-feasibility-spikes`, supaya bisa dikerjakan dan dilacak lewat `/opsx:apply`. M0.1 menjadi bagian dari change `setup-dev-environment`.

**Yang tetap:**
- Kode spike tetap di `spikes/sN-*`, dikecualikan dari workspace Cargo, dan tidak diimpor oleh crate produksi.
- Hasil tetap dicatat di `docs/SPIKES.md` dengan format baku: Status (PASS/FAIL/PARTIAL), versi, nama API/fungsi (V-x), bukti, dampak desain.
- Penomoran folder dan perluasan DoD M0.1/M0.4/M0.6 di atas.

**Tambahan:** semua spike dikerjakan di regtest laptop lokal. Bagian live M0.5 (V-27 di node live) ditandai "deferred to M12" (ADR-0012).

## Consequences

- Spec delta change spike hanya menyentuh capability `dev-environment` (cakupan V-item, go/no-go, tooling FROST lokal, sync escrow, isolasi kode spike, spike yang menunggu manusia).
- `setup-dev-environment` harus diarsipkan sebelum `run-feasibility-spikes`.
- Hasil spike yang bertentangan dengan spec tetap harus dicatat sebagai ADR baru dan spec diperbarui (§28).
- §36 diperluas dengan V-20..V-29 untuk item `[TO VERIFY]` yang sebelumnya tidak terkonsolidasi.
