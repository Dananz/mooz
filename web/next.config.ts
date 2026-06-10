import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { NextConfig } from "next";

// Single source of truth: the repo-root VERSION file. Read at build time and
// baked into the bundle, so the site's displayed version can never drift from
// the app/release - rebuild the site and it picks up the new VERSION.
const appVersion = readFileSync(join(process.cwd(), "..", "VERSION"), "utf8").trim();

// Served at the root of the custom domain (mooz.dananz.com), so no basePath.
const nextConfig: NextConfig = {
  output: "export",
  trailingSlash: true,
  images: { unoptimized: true },
  env: { NEXT_PUBLIC_APP_VERSION: appVersion },
  // Allow the dev server to serve _next/* assets when reached over the tailnet
  // (Next 16 blocks cross-origin dev requests otherwise -> JS never loads).
  allowedDevOrigins: [
    "tomers-mac-studio",
    "tomers-mac-studio.tail08ad7c.ts.net",
    "100.116.236.71",
  ],
};

export default nextConfig;
