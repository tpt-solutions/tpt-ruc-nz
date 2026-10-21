import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import "./globals.css";
import { ServiceWorkerRegister } from "./sw-register";

export const metadata: Metadata = {
    title: "tpt-ruc-nz",
    description: "Offline-first RUC tracker. Open source, not an NZTA-approved distance recorder.",
    manifest: "/manifest.webmanifest",
};

export const viewport: Viewport = {
    themeColor: "#1f2937",
    width: "device-width",
    initialScale: 1,
};

export default function RootLayout({ children }: { children: ReactNode }) {
    return (
        <html lang="en-NZ">
            <body className="min-h-dvh bg-neutral-50 text-neutral-900 antialiased">
                <main className="mx-auto max-w-2xl px-4 py-8">{children}</main>
                <ServiceWorkerRegister />
                <footer className="mx-auto max-w-2xl px-4 pb-8 text-sm text-neutral-600">
                    <p>
                        Not an NZTA-approved distance recorder. Claiming RUC deductions or using a
                        digital display in place of a licence label requires Section 43 approval
                        from NZTA first.
                    </p>
                </footer>
            </body>
        </html>
    );
}
