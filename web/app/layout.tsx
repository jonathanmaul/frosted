import type { Metadata } from "next";
import Link from "next/link";
import type { ReactNode } from "react";

import { navScreens } from "@/lib/screens";

import "./globals.css";

export const metadata: Metadata = {
  title: "Frosted",
  description: "Public odds. Invisible bettors.",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>
        <header className="border-b border-slate-200">
          <nav className="mx-auto flex max-w-5xl flex-wrap items-center gap-4 px-4 py-3">
            <Link href="/" className="font-bold tracking-wide">
              FROSTED
            </Link>
            {navScreens.map((s) => (
              <Link key={s.id} href={s.href} className="text-slate-600 hover:text-slate-900">
                {s.title}
              </Link>
            ))}
          </nav>
        </header>
        <main className="mx-auto max-w-5xl px-4 py-8">{children}</main>
      </body>
    </html>
  );
}
