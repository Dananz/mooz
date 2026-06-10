import { RELEASES, REPO, VERSION } from "@/content";

export function Footer() {
  return (
    <footer className="border-t border-line py-10">
      <div className="flex flex-col gap-3 font-mono text-xs text-muted sm:flex-row sm:items-center sm:justify-between">
        <p>
          Mooz is an independent project. Not affiliated with Apple or Logitech.
          MIT licensed. © Dananz.
        </p>
        <div className="flex items-center gap-4">
          <a
            href={RELEASES}
            className="text-steel transition-colors hover:text-text"
          >
            v{VERSION}
          </a>
          <a
            href={REPO}
            className="text-steel transition-colors hover:text-text"
          >
            GitHub
          </a>
        </div>
      </div>
    </footer>
  );
}
