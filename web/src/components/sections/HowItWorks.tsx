import Reveal from "@/components/Reveal";
import ZoomClip from "@/components/ZoomClip";

export function HowItWorks() {
  return (
    <section className="border-b border-line py-20 sm:py-28">
      <Reveal className="text-center" armDelay={2000}>
        <p className="font-display text-xs font-bold uppercase tracking-[0.3em] text-signal">
          How it works
        </p>
        <h2 className="mt-4 font-display text-[clamp(2rem,5.5vw,3.5rem)] font-bold uppercase leading-[0.95] tracking-tight">
          Hold. Move. Zoom.
        </h2>
      </Reveal>

      <Reveal delay={0.1}>
        <div className="mt-12 flex flex-col items-center">
          {/* Faux browser window: the demo "works in any app" so we frame it like one. */}
          <div className="w-full max-w-[560px] overflow-hidden border border-line">
            {/* Title bar with a faux URL */}
            <div className="flex items-center gap-2 border-b border-line bg-ink/60 px-3 py-2">
              <div className="flex shrink-0 items-center gap-1.5">
                <span className="h-2.5 w-2.5 rounded-full bg-[#ff5f57]/70" />
                <span className="h-2.5 w-2.5 rounded-full bg-[#febc2e]/70" />
                <span className="h-2.5 w-2.5 rounded-full bg-[#28c840]/70" />
              </div>
              <span className="flex-1 truncate rounded bg-panel-2 px-3 py-1 text-center font-mono text-[11px] text-muted">
                example.com
              </span>
              <div className="w-[42px] shrink-0" aria-hidden />
            </div>

            {/* Auto-playing zoom clip - hands-free demo of the cursor-anchored zoom. */}
            <ZoomClip />
          </div>

          <p className="mt-4 max-w-[44ch] text-center font-body text-sm leading-relaxed text-muted">
            Shift or Cmd, drag or scroll - whatever feels right. Point at the
            small stuff and pull it close.
          </p>
        </div>
      </Reveal>
    </section>
  );
}
