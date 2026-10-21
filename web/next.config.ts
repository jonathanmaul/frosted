import type { NextConfig } from "next";

// NFR-P2: no third-party requests at runtime. No remote images, no external fonts.
const nextConfig: NextConfig = {
  poweredByHeader: false,
  images: { unoptimized: true },
};

export default nextConfig;
