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

export const metadata: Metadata = {
  title: "Mooz - pinch-to-zoom for any mouse",
  description:
    "Mooz makes any mouse behave like a trackpad - hold a key, move, and any app zooms with a real pinch gesture, even in Firefox and Zen. A tiny macOS menu-bar app.",
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
