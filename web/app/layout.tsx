import "@fontsource/instrument-serif/400.css";
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
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
        <Background />
        <header className="relative z-10 border-b border-[var(--border)]">
          <nav className="mx-auto grid max-w-[1280px] grid-cols-12 items-center gap-4 px-6 py-4" aria-label="Main">
            <Link href="/" className="num col-span-3 t-4 font-medium tracking-[0.3em]">
              FROSTED
            </Link>
            <div className="col-span-6 flex flex-wrap gap-6 t-5">
              {navScreens.slice(1).map((s) => (
                <Link key={s.id} href={s.href} className="text-[var(--ice-muted)] hover:text-[var(--ice)]">
                  {s.title}
                </Link>
              ))}
            </div>
            <div className="col-span-3 text-right">
              <BlockTicker />
            </div>
          </nav>
        </header>
        <main className="relative z-10 mx-auto max-w-[1280px] px-6">{children}</main>
      </body>
    </html>
  );
}
