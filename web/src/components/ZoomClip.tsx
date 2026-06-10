"use client";

import { motion, useTime, useTransform, cubicBezier } from "motion/react";

/**
 * Auto-playing, looping demo of Mooz's core behavior: hold the modifier and drag
 * (up to zoom in, down to zoom out) while the cursor stays pinned. The fine print
 * on a mock webpage is an illegible blur until the view zooms into it.
 *
 * EVERYTHING is driven by ONE clock (`useTime` -> normalized loop progress `p`),
 * so the scene scale, cursor, chip, readout and press-ring can never drift out of
 * sync (separate repeating animations did, which made the chip vanish mid-loop).
 * The cursor slides in, stays pinned + visible through the whole zoom, then slides
 * out and fades only at rest - so the "Shift + drag" chip never disappears mid-zoom.
 */

const FOCAL_X = "50%";
const FOCAL_Y = "68%";
const MAX_SCALE = 3.1;
const DURATION = 12; // seconds per loop (two 6s rounds)

const linear = (t: number) => t;
const zoomIn = cubicBezier(0.65, 0, 0.35, 1); // smooth ease-in-out into the apex
const zoomOut = cubicBezier(0.65, 0, 0.35, 1); // smooth ease-in-out back to rest

// The loop runs TWO rounds (same zoom choreography), each on half the loop:
//   round 1: "Shift + drag"   round 2: "Cmd + scroll"
// showing that the modifier AND the input source are configurable. All motion is
// driven by the half-progress `h`, the chip swap by the full progress `p` - one
// clock, nothing can drift.

export default function ZoomClip() {
  const time = useTime();
  const p = useTransform(time, (t) => (t % (DURATION * 1000)) / (DURATION * 1000));
  // Half-progress: 0..1 within each round.
  const h = useTransform(p, (v) => (v % 0.5) * 2);

  // ~0.5s pause at the apex between drag-up and drag-down. (A 200ms plateau got
  // swallowed by the ease tails - it read as no pause at all.)
  const scale = useTransform(
    h,
    [0, 0.1, 0.32, 0.405, 0.62, 1],
    [1, 1, MAX_SCALE, MAX_SCALE, 1, 1],
    { ease: [linear, zoomIn, linear, zoomOut, linear] },
  );

  // Cursor: slides in from down-left, pinned through both zooms, slides out + fades
  // at the end of each round (so the chip swap happens while it's hidden).
  const curX = useTransform(h, [0, 0.1, 0.66, 0.76], [26, 0, 0, 16], {
    ease: [zoomOut, linear, zoomIn],
  });
  const curY = useTransform(h, [0, 0.1, 0.66, 0.76], [26, 0, 0, 16], {
    ease: [zoomOut, linear, zoomIn],
  });
  const curOpacity = useTransform(h, [0, 0.06, 0.68, 0.76], [0, 1, 1, 0]);

  const readoutOpacity = useTransform(
    h,
    [0, 0.26, 0.32, 0.5, 0.58],
    [0, 0, 1, 1, 0],
  );
  const ringScale = useTransform(h, [0, 0.06, 0.13, 0.22], [0.4, 0.4, 1.5, 1.7]);
  const ringOpacity = useTransform(h, [0, 0.06, 0.13, 0.22], [0, 0, 0.55, 0]);
  // Chevron points up during zoom-in (drag/scroll up), flips down for zoom-out.
  const chevronRotate = useTransform(h, [0, 0.34, 0.39, 1], [0, 0, 180, 180]);

  // Chip swap: round 1 shows chip A, round 2 chip B. The flip lands at p=0.5 /
  // p=1, where the cursor (their parent) is already faded out.
  const chipAOpacity = useTransform(p, (v) => (v < 0.5 ? 1 : 0));
  const chipBOpacity = useTransform(p, (v) => (v < 0.5 ? 0 : 1));

  return (
    <div className="relative aspect-[4/3] w-full select-none overflow-hidden bg-panel-2">
      {/* Scaled scene. Scales around the focal point; cursor + chip stay pinned. */}
      <motion.div
        className="absolute inset-0"
        style={{
          scale,
          transformOrigin: `${FOCAL_X} ${FOCAL_Y}`,
          willChange: "transform",
        }}
      >
        <PageScene />
      </motion.div>

      {/* Cursor overlay, anchored at the focal point. */}
      <div
        className="pointer-events-none absolute"
        style={{ left: FOCAL_X, top: FOCAL_Y }}
        aria-hidden
      >
        {/* Press ring: a quick pulse synced to the zoom-in start. */}
        <motion.span
          className="absolute -left-2 -top-2 block h-9 w-9 rounded-full border border-signal"
          style={{ scale: ringScale, opacity: ringOpacity, willChange: "transform, opacity" }}
        />

        {/* Cursor + chip move together (chip rides the cursor's opacity). */}
        <motion.div
          className="absolute left-0 top-0"
          style={{ x: curX, y: curY, opacity: curOpacity, willChange: "transform, opacity" }}
        >
          <CursorArrow />
          {/* Round 1: drag */}
          <motion.span
            style={{ opacity: chipAOpacity }}
            className="absolute left-[16px] top-[22px] flex h-7 items-center gap-1 whitespace-nowrap rounded-md bg-signal px-2.5 font-mono text-[13px] font-bold leading-none text-signal-ink shadow-[0_2px_10px_rgba(255,176,32,0.5)]"
          >
            Shift + drag
            <Chevron rotate={chevronRotate} />
          </motion.span>
          {/* Round 2: same zoom, different modifier + input source */}
          <motion.span
            style={{ opacity: chipBOpacity }}
            className="absolute left-[16px] top-[22px] flex h-7 items-center gap-1 whitespace-nowrap rounded-md bg-signal px-2.5 font-mono text-[13px] font-bold leading-none text-signal-ink shadow-[0_2px_10px_rgba(255,176,32,0.5)]"
          >
            Cmd + scroll
            <Chevron rotate={chevronRotate} />
          </motion.span>
        </motion.div>
      </div>

      {/* Zoom readout, top-right. Visible while the view is zoomed (same clock). */}
      <motion.span
        className="pointer-events-none absolute right-3 top-3 bg-ink/85 px-2 py-1 font-mono text-xs font-medium text-signal"
        style={{ opacity: readoutOpacity }}
        aria-hidden
      >
        {MAX_SCALE.toFixed(1)}&#215;
      </motion.span>
    </div>
  );
}

// Direction chevron inside the chips - up while zooming in, down while zooming out.
function Chevron({ rotate }: { rotate: ReturnType<typeof useTransform<number, number>> }) {
  return (
    <motion.svg
      width="11"
      height="11"
      viewBox="0 0 12 12"
      fill="none"
      style={{ rotate, transformOrigin: "center" }}
    >
      <path
        d="M2.5 7.5 L6 4 L9.5 7.5"
        stroke="currentColor"
        strokeWidth="1.9"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </motion.svg>
  );
}

// A clean macOS-style arrow pointer. Tip sits at the SVG's (0,0) = the focal point.
function CursorArrow() {
  return (
    <svg
      width="22"
      height="32"
      viewBox="0 0 22 32"
      fill="none"
      style={{ filter: "drop-shadow(0 2px 3px rgba(0,0,0,0.55))" }}
    >
      <path
        d="M1 1 L1 23 L6.6 18 L10 26.5 L13.4 25 L10 16.5 L17 16.5 Z"
        fill="#ffffff"
        stroke="#1a1206"
        strokeWidth="1.2"
        strokeLinejoin="round"
      />
    </svg>
  );
}

// A mock webpage. Most of it is placeholder "lorem" bars; the payoff is the
// fine-print card in the lower middle, whose tiny real text is an illegible blur
// at 1x and crisply readable once the clip zooms in. viewBox is 480x360 (4:3).
function PageScene() {
  return (
    <svg
      viewBox="0 0 480 360"
      width="100%"
      height="100%"
      preserveAspectRatio="xMidYMid slice"
      role="presentation"
    >
      <rect x="0" y="0" width="480" height="360" fill="#0d1016" />

      {/* top nav */}
      <rect x="24" y="18" width="13" height="13" rx="3" fill="#ffb020" />
      <rect x="44" y="22" width="34" height="5" rx="2.5" fill="#ecedf1" opacity="0.85" />
      <g fill="#8a91a0">
        <rect x="356" y="22" width="24" height="5" rx="2.5" />
        <rect x="388" y="22" width="24" height="5" rx="2.5" />
      </g>
      <rect x="420" y="17" width="36" height="15" rx="7.5" fill="#ffb020" />
      <line x1="24" y1="44" x2="456" y2="44" stroke="#1c212b" strokeWidth="1" />

      {/* hero headline (left) */}
      <rect x="24" y="70" width="190" height="15" rx="3.5" fill="#ecedf1" opacity="0.92" />
      <rect x="24" y="92" width="140" height="15" rx="3.5" fill="#ffb020" opacity="0.9" />
      {/* hero subtext */}
      <g fill="#8a91a0">
        <rect x="24" y="120" width="200" height="6" rx="3" />
        <rect x="24" y="132" width="176" height="6" rx="3" />
      </g>
      {/* hero CTA */}
      <rect x="24" y="150" width="74" height="20" rx="4" fill="#ffb020" />
      <rect x="106" y="155" width="50" height="10" rx="5" fill="none" stroke="#2a3140" strokeWidth="1.5" />

      {/* hero media card (right) */}
      <defs>
        <linearGradient id="mz-media" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#ffb020" stopOpacity="0.28" />
          <stop offset="1" stopColor="#8fb4c4" stopOpacity="0.18" />
        </linearGradient>
      </defs>
      <rect x="288" y="66" width="168" height="104" rx="8" fill="url(#mz-media)" stroke="#1c212b" strokeWidth="1" />
      <circle cx="372" cy="118" r="14" fill="#0d1016" opacity="0.55" />
      <path d="M368 112 L380 118 L368 124 Z" fill="#ecedf1" />

      {/* three feature bars */}
      <g>
        <rect x="24" y="196" width="136" height="40" rx="6" fill="#11141b" stroke="#1c212b" strokeWidth="1" />
        <rect x="320" y="196" width="136" height="40" rx="6" fill="#11141b" stroke="#1c212b" strokeWidth="1" />
        <g fill="#2a3140">
          <rect x="34" y="206" width="60" height="6" rx="3" />
          <rect x="34" y="218" width="96" height="5" rx="2.5" />
          <rect x="330" y="206" width="60" height="6" rx="3" />
          <rect x="330" y="218" width="96" height="5" rx="2.5" />
        </g>
      </g>

      {/* FOCAL: the fine-print card. Center ~ (240, 246) -> 50% / 68%. */}
      <rect x="150" y="200" width="180" height="92" rx="8" fill="#10131a" stroke="#262b36" strokeWidth="1.2" />
      <text
        x="162"
        y="220"
        fill="#ffb020"
        fontFamily="ui-monospace, Menlo, monospace"
        fontSize="7"
        fontWeight="700"
        letterSpacing="0.8"
      >
        THE FINE PRINT
      </text>
      <g fill="#cfd3da" fontFamily="ui-monospace, Menlo, monospace" fontSize="5.2" letterSpacing="0.2">
        <text x="162" y="235">Mooz zooms any pixel on screen,</text>
        <text x="162" y="246">even text this small. No squinting,</text>
        <text x="162" y="257">no clumsy Cmd-plus. Hold, and move.</text>
      </g>
      <text
        x="162"
        y="276"
        fill="#8fb4c4"
        fontFamily="ui-monospace, Menlo, monospace"
        fontSize="4.6"
        letterSpacing="0.2"
      >
        you found the fine print. nicely zoomed.
      </text>
    </svg>
  );
}
