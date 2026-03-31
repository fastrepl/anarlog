import { AnimatePresence, motion, useInView } from "motion/react";
import { useEffect, useRef, useState } from "react";

import { cn } from "@hypr/utils";

type Logo = {
  src: string;
  alt: string;
};

const LOGOS: Logo[][] = [
  [
    { src: "/icons/databricks.svg", alt: "Databricks Logo" },
    { src: "/icons/amazon.svg", alt: "Amazon Logo" },
  ],
  [
    { src: "/icons/meta.svg", alt: "Meta Logo" },
    { src: "/icons/palantir.svg", alt: "Palantir Logo" },
  ],
  [
    { src: "/icons/apple.svg", alt: "Apple Logo" },
    { src: "/icons/disney.svg", alt: "Disney Logo" },
  ],
  [
    { src: "/icons/richmond_american.svg", alt: "Richmond American Logo" },
    { src: "/icons/adobe.svg", alt: "Adobe Logo" },
  ],
  [
    { src: "/icons/wayfair.svg", alt: "Wayfair Logo" },
    { src: "/icons/bain.svg", alt: "Bain Logo" },
  ],
];

const CYCLE_INTERVAL = 3000;
const STAGGER_DELAY = 300;

function LogoSlot({ logos, delay }: { logos: Logo[]; delay: number }) {
  const [index, setIndex] = useState(0);
  const ref = useRef<HTMLDivElement>(null);
  const isInView = useInView(ref, { amount: 0.4 });

  useEffect(() => {
    if (!isInView) {
      return;
    }

    let interval: ReturnType<typeof setInterval>;
    const timeout = setTimeout(() => {
      interval = setInterval(() => {
        setIndex((prev) => (prev + 1) % logos.length);
      }, CYCLE_INTERVAL);
    }, delay);
    return () => {
      clearTimeout(timeout);
      clearInterval(interval);
    };
  }, [delay, isInView, logos.length]);

  const logo = logos[index];

  return (
    <div
      ref={ref}
      className={cn([
        "relative flex h-20 items-center justify-center overflow-hidden",
      ])}
    >
      <AnimatePresence mode="wait">
        <motion.img
          key={logo.src}
          src={logo.src}
          alt={logo.alt}
          className="pointer-events-none h-5 select-none md:h-6"
          initial={{ y: -30, opacity: 0, filter: "blur(8px)" }}
          animate={{ y: 0, opacity: 1, filter: "blur(0px)" }}
          exit={{ y: 30, opacity: 0, filter: "blur(8px)" }}
          transition={{ duration: 0.5, ease: "easeInOut" }}
        />
      </AnimatePresence>
    </div>
  );
}

export function LogoCloud() {
  return (
    <div className="relative grid grid-cols-2 items-center justify-center gap-2 md:grid-cols-5 md:gap-0">
      <div className="pointer-events-none absolute -top-px left-1/2 w-full -translate-x-1/2" />
      {LOGOS.map((pair, i) => (
        <LogoSlot key={i} logos={pair} delay={i * STAGGER_DELAY} />
      ))}
      <div className="pointer-events-none absolute -bottom-px left-1/2 w-full -translate-x-1/2" />
    </div>
  );
}
