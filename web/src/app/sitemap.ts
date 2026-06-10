import type { MetadataRoute } from "next";

export const dynamic = "force-static";

const siteUrl =
  process.env.NEXT_PUBLIC_PAGES === "true"
    ? "https://mooz.dananz.com"
    : "http://localhost:3000";

export default function sitemap(): MetadataRoute.Sitemap {
  return [
    {
      url: `${siteUrl}/`,
      changeFrequency: "monthly",
      priority: 1,
    },
  ];
}
