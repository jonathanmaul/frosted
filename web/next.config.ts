import type { NextConfig } from "next";

// NFR-P2: no third-party requests at runtime. No remote images, no external fonts.
// The browser enforces it: the CSP only allows this origin and the Frosted API.
const apiOrigin = (() => {
  try { return new URL(process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080/v1").origin; } catch { return "http://localhost:8080"; }
})();
const dev = process.env.NODE_ENV !== "production";

const csp = [
  "default-src 'self'",
  // Next.js inlines its bootstrap scripts; WASM needs 'wasm-unsafe-eval'. React Refresh needs eval in dev only.
  `script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'${dev ? " 'unsafe-eval'" : ""}`,
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: blob:",
  "font-src 'self'",
  `connect-src 'self' ${apiOrigin}${dev ? " ws:" : ""}`,
  "worker-src 'self' blob:",
  "object-src 'none'",
  "base-uri 'self'",
  "form-action 'self'",
  "frame-ancestors 'none'",
].join("; ");

const nextConfig: NextConfig = {
  poweredByHeader: false,
  // A separate build directory lets an end-to-end run build without clobbering `next dev`.
  distDir: process.env.NEXT_DIST_DIR ?? ".next",
  images: { unoptimized: true },
  async headers() {
    return [{
      source: "/:path*",
      headers: [
        { key: "Content-Security-Policy", value: csp },
        // The browser never tells another site which market or Persona a visitor looked at.
        { key: "Referrer-Policy", value: "no-referrer" },
        { key: "X-Content-Type-Options", value: "nosniff" },
        { key: "X-Frame-Options", value: "DENY" },
        { key: "Cross-Origin-Opener-Policy", value: "same-origin" },
        { key: "Permissions-Policy", value: "camera=(), microphone=(), geolocation=(), payment=(), usb=(), interest-cohort=()" },
      ],
    }];
  },
};

export default nextConfig;
