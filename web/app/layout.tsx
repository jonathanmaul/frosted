import "@fontsource/instrument-serif/latin-400.css";
import "@fontsource/instrument-serif/latin-400-italic.css";
import "@fontsource/ibm-plex-sans/latin-400.css";
import "@fontsource/ibm-plex-sans/latin-500.css";
import "@fontsource/ibm-plex-mono/latin-400.css";
import "@fontsource/ibm-plex-mono/latin-500.css";
import "./globals.css";

import type { Metadata } from "next";
import Link from "next/link";
import type { ReactNode } from "react";

import { Background } from "@/components/ui/Background";
import { BlockTicker } from "@/components/ui/BlockTicker";
import { navScreens } from "@/lib/screens";

export const metadata: Metadata = {
  title: "Frosted",
  description: "Public odds. Invisible bettors.",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body className="min-h-screen">
        <a className="skip-link" href="#main-content">Skip to content</a>
        <Background />
        {/* Persistent shell follows docs/DESIGN.md §4. */}
        <header className="site-header">
          <nav className="site-nav" aria-label="Main">
            <Link href="/" className="wordmark" aria-label="Frosted home">FROSTED<span className="wordmark-period" aria-hidden="true">.</span></Link>
            <div className="nav-links">
              {navScreens.map((s) => <Link key={s.id} href={s.id === "S-01" ? "/#markets" : s.href}>{s.title}</Link>)}
            </div>
            <div className="header-ticker"><span className="ticker-indicator" aria-hidden="true" /><BlockTicker /></div>
          </nav>
        </header>
        <main id="main-content" className="site-main">{children}</main>
        <footer className="site-footer">
          <Link href="/" className="wordmark">FROSTED.</Link>
          <p className="num t-5">PUBLIC ODDS. INVISIBLE BETTORS.</p>
          <div className="flex gap-6 t-5"><Link href="/how">Trust & privacy</Link><span>Research demo / Zcash</span></div>
        </footer>
      </body>
    </html>
  );
}
