# Models

`frost-poster.webp` is the static fallback for the landing hero (SSR, reduced motion, weak hardware, WebGL failure): a local render of the procedural scene in `components/landing/HeroScene.tsx`. All 3D is generated in code; there are no downloaded models or textures. Regenerate the poster by screenshotting the landing with the opening text hidden.

See `docs/DESIGN.md` §14 and `docs/decisions/0019-paper-redesign.md`.
