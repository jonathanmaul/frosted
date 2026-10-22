# ADR-0017: Through the Frost opening and scroll journey

Status: accepted by user request, 2026-10-22; refined by subsequent user instructions in the same session.

The user rejected the conventional two-column marketing hero and requested centered opening text followed immediately by an immersive scroll journey. This explicitly supersedes the original hero composition in `../DESIGN.md` §4 for the landing opening. The other palette, typography, accessibility, and privacy rules remain in force.

One full-viewport Canvas displays a generated ice sculpture behind the opening. Scroll fades the introduction and drives a reversible camera path through the sculpture into a procedural ledger world. Four mechanism chapters appear individually. Desktop pin distance is 4.4 viewport heights; compact devices use 3. Keyboard chapter controls and a skip link provide direct navigation.

The user also explicitly permitted a generated GLB. `web/scripts/generate-frost-model.mjs` creates the original self-contained `web/public/models/frost-ledger.glb` (about 61 KB) without third-party assets. The scene adds live odds, permanent silhouette blur, micro-frost, and studio lighting. No new dependency or second WebGL context is introduced.

The enhancement starts only after 3D is available. Reduced motion, weak devices, short viewports, loading failures, and WebGL failures keep the full linear narrative. Headless Chrome checks cover loading, desktop traversal, reversing, mobile layout, and changing reduced-motion preference. Hardware GPU performance and Lighthouse still require separate measurement.
