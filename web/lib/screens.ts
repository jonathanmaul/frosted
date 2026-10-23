// Screen list from docs/PROJECT_SPEC_FROSTED.md §16. Routes from §26.
export type Screen = { id: string; title: string; href: string; features: string };

export const navScreens: Screen[] = [
  { id: "S-01", title: "Markets", href: "/markets", features: "F-04" },
  { id: "S-09", title: "Leaderboard", href: "/leaderboard", features: "F-10" },
  { id: "S-04", title: "My Bets", href: "/my-bets", features: "F-02, F-12" },
  { id: "S-06", title: "Persona", href: "/persona", features: "F-09" },
  { id: "S-12", title: "How it works", href: "/how", features: "—" },
];
