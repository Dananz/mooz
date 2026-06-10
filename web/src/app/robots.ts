import type { MetadataRoute } from "next";

export const dynamic = "force-static";

const siteUrl =
  process.env.NEXT_PUBLIC_PAGES === "true"
    ? "https://mooz.dananz.com"
    : "http://localhost:3000";

export default function robots(): MetadataRoute.Robots {
  return {
    rules: [{ userAgent: "*", allow: "/" }],
    sitemap: `${siteUrl}/sitemap.xml`,
  };
}
