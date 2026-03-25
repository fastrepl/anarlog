import { Link } from "@tanstack/react-router";
import {
  AnimatePresence,
  motion,
  useMotionValueEvent,
  useScroll,
} from "motion/react";
import { useEffect, useState } from "react";

import { getPlatformCTA, usePlatform } from "@/hooks/use-platform";

export function RightPanel({ isHomePage }: { isHomePage: boolean }) {
  const platform = usePlatform();
  const platformCTA = getPlatformCTA(platform);

  const { scrollY } = useScroll();
  const [showCTA, setShowCTA] = useState(!isHomePage);

  useMotionValueEvent(scrollY, "change", (latest) => {
    if (!isHomePage) {
      setShowCTA(true);
      return;
    }
    setShowCTA(latest > window.innerHeight);
  });

  useEffect(() => {
    setShowCTA(!isHomePage);
  }, [isHomePage]);

  const baseClass =
    "flex h-9 items-center justify-center rounded-lg bg-neutral-800 text-sm text-neutral-300 transition-colors hover:bg-neutral-700 hover:text-neutral-100";

  return (
    <aside className="wide:w-[200px] z-10 hidden w-[120px] shrink-0 self-stretch xl:block">
      <div className="sticky top-0 flex h-screen flex-col justify-start">
        <div className="wide:px-8 shrink-0 px-4 pt-12">
          <AnimatePresence>
            {showCTA && (
              <motion.div
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: 8 }}
                transition={{ duration: 0.2 }}
              >
                {platformCTA.action === "download" ? (
                  <a
                    href="/download/apple-silicon"
                    download
                    className={baseClass}
                  >
                    {platformCTA.label}
                  </a>
                ) : (
                  <Link to="/" className={baseClass}>
                    {platformCTA.label}
                  </Link>
                )}
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>
    </aside>
  );
}
