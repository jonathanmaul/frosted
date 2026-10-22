# Frost ledger

Original procedural ice sculpture created for Frosted. No downloaded model, remote texture, or third-party model license is required.

- `frost-ledger.glb`: self-contained GLB 2.0 with beveled asymmetric ice, rim facets, suspended folios, calibration cuts, and fissures. Regenerate from `web/` with `node scripts/generate-frost-model.mjs`.
- `frost-ledger-static.webp`: local browser render of the same sculpture for SSR, reduced motion, weak hardware, and WebGL failure.

`IceShell` receives the runtime frost/transmission shader. Live odds and permanently blurred silhouettes are rendered separately so the asset contains no invented market data or identities.

See `docs/DESIGN.md` and ADR-0017 for the permanent visual brief and the opening/scroll sequence.
