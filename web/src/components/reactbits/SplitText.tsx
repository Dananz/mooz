"use client";

import { motion } from "motion/react";

type SplitTextProps = {
  text: string;
  className?: string;
  splitType?: "chars" | "words";
  duration?: number;
  /** seconds between each animated unit */
  stagger?: number;
  /** seconds before the first unit starts (use to sequence in a timeline) */
  startDelay?: number;
  yFrom?: number;
  tag?: "h1" | "h2" | "p" | "span" | "div";
};

const EASE: [number, number, number, number] = [0.22, 1, 0.36, 1];

// SplitText effect (chars/words rise + fade in, staggered) built on `motion`
// instead of gsap - same look, no extra deps. Wraps words so the headline
// still wraps naturally; chars within a word stay together.
export default function SplitText({
  text,
  className = "",
  splitType = "chars",
  duration = 0.5,
  stagger = 0.02,
  startDelay = 0,
  yFrom = 40,
  tag = "p",
}: SplitTextProps) {
  const Tag = tag;
  const words = text.split(" ");
  let unit = 0;

  return (
    <Tag
      aria-label={text}
      className={`flex flex-wrap ${className}`}
      style={{ columnGap: "0.26em", rowGap: "0.08em" }}
    >
      {words.map((word, wi) => {
        if (splitType === "words") {
          const i = unit++;
          return (
            <motion.span
              key={wi}
              aria-hidden
              className="inline-block"
              initial={{ opacity: 0, y: yFrom }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration, delay: startDelay + i * stagger, ease: EASE }}
            >
              {word}
            </motion.span>
          );
        }
        return (
          <span key={wi} aria-hidden className="inline-flex whitespace-nowrap">
            {Array.from(word).map((ch, ci) => {
              const i = unit++;
              return (
                <motion.span
                  key={ci}
                  className="inline-block"
                  initial={{ opacity: 0, y: yFrom }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{
                    duration,
                    delay: startDelay + i * stagger,
                    ease: EASE,
                  }}
                >
                  {ch}
                </motion.span>
              );
            })}
          </span>
        );
      })}
    </Tag>
  );
}
