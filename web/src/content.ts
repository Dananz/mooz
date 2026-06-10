export const REPO = "https://github.com/Dananz/mooz";
// Direct download of the latest release's DMG. The constant asset name + the
// `latest/download` redirect mean this always fetches the newest Mooz.dmg and
// triggers the download immediately (no releases page in between).
export const DOWNLOAD =
  "https://github.com/Dananz/mooz/releases/latest/download/Mooz.dmg";
// Human-facing releases page (changelog), used by the footer version link.
export const RELEASES = "https://github.com/Dananz/mooz/releases/latest";
// Injected at build time from the repo-root VERSION file (see next.config.ts).
// Never hardcode a version here - that's what caused drift.
export const VERSION = process.env.NEXT_PUBLIC_APP_VERSION ?? "dev";
