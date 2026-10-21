import type { MetadataRoute } from "next";

export const dynamic = "force-static";

export default function manifest(): MetadataRoute.Manifest {
    return {
        name: "tpt-ruc-nz",
        short_name: "RUC",
        description: "Offline-first RUC tracker. Not an NZTA-approved distance recorder.",
        start_url: "/",
        display: "standalone",
        background_color: "#fafafa",
        theme_color: "#1f2937",
        icons: [{ src: "/icon.svg", sizes: "any", type: "image/svg+xml" }],
    };
}
