# ADR-0010: Crate librustzcash upstream lebih dulu, fork Zakura hanya dengan bukti

- Status: Accepted
- Tanggal: 2026-10-21
- Bagian spec: §14 (dependensi), §19.2 butir 4, §25, §28 (output M0)

## Context

Spec versi 0.1 mencampur referensi upstream dan fork: §19.2 butir 4 menyebut tipe `IronwoodFvk` ada di test suite `zcash_client_backend`, tetapi tautannya mengarah ke crate fork `zakura-client-sqlite`. Mencampur dua kelompok crate berisiko konflik versi dan dukungan jangka panjang yang tidak jelas.

## Decision

- Default: crate librustzcash **upstream** dari crates.io.
- Fork Zakura hanya dipakai jika spike membuktikan fitur Ironwood yang dibutuhkan tidak ada di upstream. Alasannya dicatat di `docs/SPIKES.md`.

## Consequences

- M0.2–M0.4 harus mencatat crate dan versi mana yang benar-benar menyediakan API Ironwood yang dipakai.
- Jika fork terpaksa dipakai, perlu ADR lanjutan yang menjelaskan batas pemakaiannya dan rencana kembali ke upstream.
