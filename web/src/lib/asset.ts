const base = process.env.NEXT_PUBLIC_PAGES === "true" ? "/mooz" : "";

export const asset = (p: string) => `${base}${p.startsWith("/") ? p : "/" + p}`;
