"use client";

import React, { useRef } from "react";
import {
  motion,
  useAnimationFrame,
  useMotionValue,
  useScroll,
  useSpring,
  useVelocity,
} from "motion/react";

interface CircularTextProps {
  text: string;
  /** seconds per full revolution at rest */
  spinDuration?: number;
  onHover?: "slowDown" | "speedUp" | "pause" | "goBonkers";
  className?: string;
}

// Scroll velocity -> extra spin. Slow scrolling adds a touch of speed; fast
// flicks spin the ring noticeably. Tuned so ~200px/s adds ~16deg/s and a hard
// flick (~3000px/s) adds ~240deg/s, capped below a blur.
const SCROLL_TO_DEG = 0.08;
const MAX_EXTRA_DEG_PER_S = 320;

const HOVER_FACTOR: Record<NonNullable<CircularTextProps["onHover"]>, number> = {
  slowDown: 0.5,
  speedUp: 4,
  pause: 0,
  goBonkers: 8,
};

const CircularText: React.FC<CircularTextProps> = ({
  text,
  spinDuration = 20,
  onHover = "speedUp",
  className = "",
}) => {
  const letters = Array.from(text);
  const rotation = useMotionValue(0);

  // Smoothed scroll velocity (px/s) - the spring keeps the speed-up/decay fluid.
  const { scrollY } = useScroll();
  const scrollVelocity = useVelocity(scrollY);
  const smoothVelocity = useSpring(scrollVelocity, {
    damping: 50,
    stiffness: 400,
  });

  // Hover factor eases toward its target each frame (no snap).
  const hoverTarget = useRef(1);
  const hoverFactor = useRef(1);

  useAnimationFrame((_, delta) => {
    const dt = delta / 1000;
    hoverFactor.current +=
      (hoverTarget.current - hoverFactor.current) * Math.min(1, dt * 6);

    const base = (360 / spinDuration) * hoverFactor.current;
    const extra = Math.min(
      Math.abs(smoothVelocity.get()) * SCROLL_TO_DEG,
      MAX_EXTRA_DEG_PER_S,
    );
    rotation.set(rotation.get() + (base + extra) * dt);
  });

  return (
    <motion.div
      className={`m-0 mx-auto rounded-full w-[200px] h-[200px] relative font-black text-center cursor-pointer origin-center ${className}`}
      style={{ rotate: rotation }}
      onMouseEnter={() => {
        hoverTarget.current = HOVER_FACTOR[onHover] ?? 1;
      }}
      onMouseLeave={() => {
        hoverTarget.current = 1;
      }}
    >
      {letters.map((letter, i) => {
        const rotationDeg = (360 / letters.length) * i;
        const factor = Math.PI / letters.length;
        const x = factor * i;
        const y = factor * i;
        const transform = `rotateZ(${rotationDeg}deg) translate3d(${x}px, ${y}px, 0)`;

        return (
          <span
            key={i}
            className="absolute inline-block inset-0 text-2xl transition-all duration-500 ease-[cubic-bezier(0,0,0,1)]"
            style={{ transform, WebkitTransform: transform }}
          >
            {letter}
          </span>
        );
      })}
    </motion.div>
  );
};

export default CircularText;
