import type { Metadata, Viewport } from "next";
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

// Production is the custom domain (served at the root). Locally it's localhost.
const isProd = process.env.NEXT_PUBLIC_PAGES === "true";
const siteUrl = isProd ? "https://mooz.dananz.com" : "http://localhost:3000";
const appVersion = process.env.NEXT_PUBLIC_APP_VERSION ?? "1.0";

const title = "Mooz - pinch-to-zoom for any mouse";
const description =
  "Mooz makes any mouse behave like a trackpad - hold a key, move, and any app zooms with a real pinch gesture, even in Firefox and Zen. A tiny macOS menu-bar app.";

export const metadata: Metadata = {
  metadataBase: new URL(siteUrl),
  title,
  description,
  applicationName: "Mooz",
  authors: [{ name: "Tomer Danan", url: "https://github.com/Dananz" }],
  creator: "Tomer Danan",
  category: "productivity",
  keywords: [
    "macOS zoom",
    "mouse pinch to zoom",
    "trackpad gesture for mouse",
    "magnify gesture",
    "zoom any app macOS",
    "menu bar app",
    "Firefox zoom with mouse",
    "accessibility zoom",
  ],
  alternates: { canonical: siteUrl },
  // App-icon renders as the favicon: light tile (dark glyph) on light browser
  // themes, dark tile (white glyph) on dark - matching the macOS app icon.
  icons: {
    icon: [
      {
        url: "/icon-light.png",
        media: "(prefers-color-scheme: light)",
        type: "image/png",
      },
      {
        url: "/icon-dark.png",
        media: "(prefers-color-scheme: dark)",
        type: "image/png",
      },
    ],
    apple: "/icon-light.png",
  },
  openGraph: {
    title,
    description,
    url: siteUrl,
    siteName: "Mooz",
    type: "website",
    locale: "en_US",
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

export const viewport: Viewport = {
  themeColor: "#0b0e14",
  colorScheme: "dark",
};

// Structured data: a free macOS app. Eligible for software-app rich results.
const jsonLd = {
  "@context": "https://schema.org",
  "@type": "SoftwareApplication",
  name: "Mooz",
  applicationCategory: "UtilitiesApplication",
  operatingSystem: "macOS 14.0 or later",
  description,
  url: `${siteUrl}/`,
  image: `${siteUrl}/og.png`,
  downloadUrl:
    "https://github.com/Dananz/mooz/releases/latest/download/Mooz.dmg",
  softwareVersion: appVersion,
  offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
  author: {
    "@type": "Person",
    name: "Tomer Danan",
    url: "https://github.com/Dananz",
  },
  license: "https://github.com/Dananz/mooz/blob/main/LICENSE",
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
        <script
          type="application/ld+json"
          // eslint-disable-next-line react/no-danger
          dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }}
        />
      </body>
    </html>
  );
}
