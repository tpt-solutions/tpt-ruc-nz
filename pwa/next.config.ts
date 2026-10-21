import type { NextConfig } from "next";

// Static export: the PWA is served as plain files, with no Node server.
const nextConfig: NextConfig = {
    output: "export",
    images: { unoptimized: true },
    reactStrictMode: true,
};

export default nextConfig;
