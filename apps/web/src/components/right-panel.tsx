import { Icon } from "@iconify-icon/react";
import { Link } from "@tanstack/react-router";
import {
  AnimatePresence,
  motion,
  useMotionValueEvent,
  useScroll,
} from "motion/react";
import { useEffect, useState } from "react";

import { DownloadButton } from "@/components/download-button";
import { getPlatformCTA, usePlatform } from "@/hooks/use-platform";

const socialLinks = [
  { href: "/x", icon: "ri:twitter-x-fill", label: "Twitter" },
  { href: "/discord", icon: "ri:discord-fill", label: "Discord" },
  { href: "/youtube", icon: "ri:youtube-fill", label: "YouTube" },
  { href: "/linkedin", icon: "ri:linkedin-fill", label: "LinkedIn" },
];

export function RightPanel({
  revealCtaOnScroll = false,
}: {
  revealCtaOnScroll?: boolean;
}) {
  const platform = usePlatform();
  const platformCTA = getPlatformCTA(platform);

  const { scrollY } = useScroll();
  const [showCTA, setShowCTA] = useState(!revealCtaOnScroll);

  useMotionValueEvent(scrollY, "change", (latest) => {
    if (!revealCtaOnScroll) {
      setShowCTA(true);
      return;
    }
    setShowCTA(latest > window.innerHeight);
  });

  useEffect(() => {
    setShowCTA(!revealCtaOnScroll);
  }, [revealCtaOnScroll]);

  const baseClass =
    "flex h-9 items-center justify-center rounded-lg bg-neutral-800 text-sm text-neutral-300 transition-colors hover:bg-neutral-700 hover:text-neutral-100";

  return (
    <aside className="wide:w-[160px] z-10 hidden w-[120px] shrink-0 self-stretch xl:block">
      <div className="sticky top-0 flex flex-col justify-start">
        <div className="wide:px-8 shrink-0 px-4 pt-12">
          <AnimatePresence>
            {showCTA && (
              <motion.div
                key="cta"
                initial={{ opacity: 0, height: 0, y: -8 }}
                animate={{ opacity: 1, height: "auto", y: 0 }}
                exit={{ opacity: 0, height: 0, y: -8 }}
                transition={{ duration: 0.25, ease: "easeInOut" }}
                style={{ overflow: "visible" }}
              >
                <div className="pb-6">
                  {platformCTA.action === "download" ? (
                    <DownloadButton variant="compact" />
                  ) : (
                    <Link to="/" className={baseClass}>
                      {platformCTA.label}
                    </Link>
                  )}
                </div>
              </motion.div>
            )}
          </AnimatePresence>
          <div className="flex flex-col items-center gap-4">
            {socialLinks.map((link) => (
              <a
                key={link.href}
                href={link.href}
                target="_blank"
                rel="noopener noreferrer"
                aria-label={link.label}
                className="text-fg-muted hover:text-color transition-colors"
              >
                <Icon icon={link.icon} width={18} height={18} />
              </a>
            ))}
          </div>
        </div>
      </div>
    </aside>
  );
}
