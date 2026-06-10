"use client";

import { motion } from "motion/react";
import { useEffect, useState, type ReactNode } from "react";

type RevealProps = {
  children: ReactNode;
  delay?: number;
  y?: number;
  duration?: number;
  /**
   * Hold hidden for this many ms after mount before the reveal may fire. Used so a
   * section that's partially in view on initial load (e.g. section 2 on mobile)
   * doesn't animate during the hero's entrance. After the delay it reveals (if in
   * view) or waits for scroll. Scroll reveals use a normal early threshold.
   */
  armDelay?: number;
  className?: string;
};

export default function Reveal({
  children,
  delay = 0,
  y = 24,
  duration = 0.8,
  armDelay = 0,
  className = "",
}: RevealProps) {
  const [armed, setArmed] = useState(armDelay === 0);

  useEffect(() => {
    if (armed) return;
    const t = setTimeout(() => setArmed(true), armDelay);
    return () => clearTimeout(t);
  }, [armed, armDelay]);

  // Until armed, hold the content hidden (no observer) so it can't reveal early.
  if (!armed) {
    return (
      <div className={className} style={{ opacity: 0 }}>
        {children}
      </div>
    );
  }

  return (
    <motion.div
      className={className}
      initial={{ opacity: 0, y }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true, amount: "some", margin: "0px 0px -10% 0px" }}
      transition={{ duration, delay, ease: [0.22, 1, 0.36, 1] }}
    >
      {children}
    </motion.div>
  );
}
