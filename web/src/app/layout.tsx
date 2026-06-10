import type { Metadata } from "next";
import { Archivo, Space_Grotesk } from "next/font/google";
import "./globals.css";

const archivo = Archivo({
  subsets: ["latin"],
  weight: ["400", "500", "600", "700", "800", "900"],
  variable: "--font-archivo",
  display: "swap",
});

const spaceGrotesk = Space_Grotesk({
  subsets: ["latin"],
  weight: ["400", "500", "700"],
  variable: "--font-space",
  display: "swap",
});

// On GitHub Pages the site is served under /mooz; locally it's at the root.
// Asset URLs (favicons, OG image) must carry the right prefix in each case.
const isPages = process.env.NEXT_PUBLIC_PAGES === "true";
const basePath = isPages ? "/mooz" : "";
const siteUrl = isPages ? "https://dananz.github.io/mooz" : "http://localhost:3000";

const title = "Mooz - pinch-to-zoom for any mouse";
const description =
  "Mooz makes any mouse behave like a trackpad - hold a key, move, and any app zooms with a real pinch gesture, even in Firefox and Zen. A tiny macOS menu-bar app.";

export const metadata: Metadata = {
  metadataBase: new URL(siteUrl),
  title,
  description,
  // App-icon renders as the favicon: light tile (dark glyph) on light browser
  // themes, dark tile (white glyph) on dark - matching the macOS app icon.
  icons: {
    icon: [
      {
        url: `${basePath}/icon-light.png`,
        media: "(prefers-color-scheme: light)",
        type: "image/png",
      },
      {
        url: `${basePath}/icon-dark.png`,
        media: "(prefers-color-scheme: dark)",
        type: "image/png",
      },
    ],
    apple: `${basePath}/icon-light.png`,
  },
  openGraph: {
    title,
    description,
    url: siteUrl,
    siteName: "Mooz",
    type: "website",
    images: [
      {
        url: `${siteUrl}/og.png`,
        width: 1200,
        height: 630,
        alt: "Mooz - pinch-to-zoom for any mouse",
      },
    ],
  },
  twitter: {
    card: "summary_large_image",
    title,
    description,
    images: [`${siteUrl}/og.png`],
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className={`${archivo.variable} ${spaceGrotesk.variable}`}>
      <body className="min-h-screen bg-ink text-text font-body antialiased">
        {children}
      </body>
    </html>
  );
}
