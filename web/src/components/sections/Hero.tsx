"use client";

import { useEffect, useState } from "react";
import { motion } from "motion/react";
import CircularText from "@/components/reactbits/CircularText";
import Magnet from "@/components/reactbits/Magnet";
import SplitText from "@/components/reactbits/SplitText";
import { DOWNLOAD, REPO } from "@/content";

const EASE: [number, number, number, number] = [0.22, 1, 0.36, 1];

// Entrance timeline: each element rises + fades in on its own beat so the
// hero assembles in sequence rather than appearing all at once.
const rise = (delay: number) => ({
  initial: { opacity: 0, y: 14 },
  animate: { opacity: 1, y: 0 },
  transition: { duration: 0.9, delay, ease: EASE },
});

export function Hero() {
  // Touch devices get no magnet effect (there's no cursor to attract toward).
  const [coarsePointer, setCoarsePointer] = useState(false);
  useEffect(() => {
    const mq = window.matchMedia("(pointer: coarse)");
    const update = () => setCoarsePointer(mq.matches);
    update();
    mq.addEventListener("change", update);
    return () => mq.removeEventListener("change", update);
  }, []);

  // Scrollbar-free viewport width for the ring clip. 100vw includes the classic
  // scrollbar (always visible on macOS when a mouse is connected), which pushed
  // the clip edge past the page-frame border.
  useEffect(() => {
    const root = document.documentElement;
    const set = () => root.style.setProperty("--cw", `${root.clientWidth}px`);
    set();
    window.addEventListener("resize", set);
    return () => window.removeEventListener("resize", set);
  }, []);

  return (
    <section className="relative border-b border-line py-20 sm:py-28">
      {/* Ring clip layer. Desktop (lg+): right edge sits on the content column's
          right edge (right-0), so the ring hugs the container, not the screen.
          Below lg the column is ~full width, so it extends to the screen edge
          (mobile) / page-frame border (sm) via --cw. overflow-hidden lives here,
          not on the section, so it never clips the CTA magnet. */}
      <div className="pointer-events-none absolute inset-y-0 left-0 z-10 overflow-hidden right-[calc((100%-var(--cw,100vw))/2)] sm:right-[calc((100%-var(--cw,100vw))/2+1.5rem)] lg:right-0">
      {/* Spinning ring opposite the CTA - last beat of the timeline. */}
      <motion.div
        initial={{ opacity: 0, scale: 0.85 }}
        animate={{ opacity: 1, scale: 1 }}
        transition={{ duration: 1.0, delay: 1.7, ease: EASE }}
        className="pointer-events-none absolute -right-14 bottom-6 lg:bottom-14 lg:right-0 [&_*]:!font-bold"
      >
        <div className="pointer-events-auto relative">
          <CircularText
            text="MOOZ · MOOZ · MOOZ · MOOZ · "
            spinDuration={22}
            onHover="speedUp"
            className="text-steel"
          />
          <span
            aria-hidden
            className="pointer-events-none absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 text-signal"
          >
            <svg
              width="40"
              height="40"
              viewBox="0 0 40 40"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <circle
                cx="17"
                cy="17"
                r="11"
                stroke="currentColor"
                strokeWidth="2.5"
              />
              <path
                d="M17 12.5V21.5M12.5 17H21.5"
                stroke="currentColor"
                strokeWidth="2.5"
                strokeLinecap="round"
              />
              <path
                d="M25.5 25.5L33 33"
                stroke="currentColor"
                strokeWidth="2.5"
                strokeLinecap="round"
              />
            </svg>
          </span>
        </div>
      </motion.div>
      </div>

      {/* headline (SplitText). sr-only h1 keeps the heading for SEO/SR. */}
      <h1 className="sr-only">Make your mouse behave like a trackpad.</h1>
      <div aria-hidden>
        <SplitText
          text="Make your mouse behave like a trackpad."
          startDelay={0.35}
          stagger={0.03}
          duration={0.75}
          className="max-w-[15ch] font-display text-[clamp(2.6rem,8vw,5.25rem)] font-bold uppercase leading-[0.92] tracking-tight text-text"
        />
      </div>

      {/* 3 - subhead */}
      <motion.p
        {...rise(0.95)}
        className="mt-7 max-w-[58ch] font-body text-lg leading-relaxed text-muted sm:text-xl"
      >
        Hold a key, move, and any app zooms - a real pinch gesture, not a
        shortcut. Works everywhere.
      </motion.p>

      {/* 4 - CTA */}
      <motion.div
        {...rise(1.2)}
        className="mt-9 flex flex-col gap-4 sm:flex-row sm:items-center sm:gap-7"
      >
        <Magnet padding={60} magnetStrength={7} disabled={coarsePointer}>
          <a
            href={DOWNLOAD}
            className="inline-flex items-center justify-center bg-signal px-7 py-3.5 font-ui text-base font-bold text-signal-ink transition-colors hover:bg-[#ffc14d]"
          >
            Download for macOS
          </a>
        </Magnet>
        <a
          href={REPO}
          className="font-ui text-base font-semibold text-steel underline decoration-1 underline-offset-4 transition-colors hover:text-text"
        >
          View source →
        </a>
      </motion.div>

      {/* 5 - meta */}
      <motion.p
        {...rise(1.45)}
        className="mt-7 font-mono text-xs text-muted"
      >
        macOS 14+
      </motion.p>
    </section>
  );
}
