# ADR-0019: Paper redesign (light palette, more 3D, cards)

Status: accepted by user request, 2026-10-25.

The user found the dark "frozen ledger" interface boring and asked for more 3D animation and a different layout and palette on every page. A prototype at `/lab` offered three palettes (Aurora, Ember, Paper); the user chose **Paper**.

## Decision

- **Light theme.** `--bg #EEF3F8`, ink `#0A1424` (the token keeps the name `--ice`), muted `#4B6078`, CTA `--accent #C93509`, new `--core #2B59FF` (section indexes, focus of the 3D scenes). `--yes #0B7A55` and `--no #C41F37` are darkened so every text token still meets WCAG AA on the new background (checked by `tests/design-system.test.tsx`).
- **One dark band per page at most** (`.band`, `--band #0A1424`) for contrast; it redefines the tokens locally so components inside flip automatically.
- **Cards and soft depth are allowed.** Market rows, the featured market and the Bet Builder are white panels with a 16px radius (`--radius-card`) and a soft shadow. A faint blue/orange radial tint sits behind the page. This replaces the "flat, no shadow, radius ≤ 4px, no gradient" rules for these surfaces; controls and tables stay flat with the 2–4px radius.
- **3D on every page.** The landing journey keeps its structure (ADR-0017) with a new scene: a faceted crystal with a pulsing core, orbiting shards, snow that parts around the cursor, live YES/NO towers and a paper-white ledger world. Every other page gets a small `Scene3D` in its header (`crystal`, `towers`, `chain`). Scenes are procedural and decorative: no information is carried by them alone.
- **No external assets.** The old `frost-ledger.glb`, its generator and the dark static poster are removed; nothing is downloaded at runtime.
- **Unchanged:** typography, the five-step type scale, copy rules, privacy rules (no bet txids, no invented data), reduced-motion/weak-device/WebGL-failure fallbacks (the scenes simply do not render), and the rule that colour never carries an outcome alone.

## Consequences

`docs/DESIGN.md` §2, §5, §9 and §14 are updated. The landing static poster is regenerated from the new scene. Hardware-GPU performance and Lighthouse still need separate measurement.
