import { Icon } from "@iconify-icon/react";
import { ArrowRight, CaretDown } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { useRef, useState } from "react";

import { DancingSticks } from "@anlg/ui/components/ui/dancing-sticks";
import { Spinner } from "@anlg/ui/components/ui/spinner";
import { cn } from "@anlg/utils";

import { AnarlogLogo } from "@/components/anarlog-logo";
import { useAnalytics } from "@/hooks/use-posthog";
import { useMountEffect } from "@/hooks/useMountEffect";
import {
  type DesktopPlatform,
  detectDesktopPlatform,
  getOrderedDesktopDownloadSections,
} from "@/lib/download";
import { getResizedImageUrl } from "@/lib/image-cdn";
import { runWhenIdle } from "@/lib/run-when-idle";
import { createTrackedTimers } from "@/lib/tracked-timers";

import { CredibilityLogoMarquee } from "./social-proof-sections";
const styles = stylex.create({
  style1: {
    isolation: "isolate",
    paddingTop: {
      default: "2.5rem",
      "@media (width >= 48rem)": "3rem",
    },
    paddingBottom: {
      default: ".5rem",
      "@media (width >= 48rem)": "1rem",
    },
  },
  style2: {
    display: "inline-flex",
  },
  style3: {
    height: {
      default: "2rem",
      "@media (width >= 48rem)": "2.25rem",
    },
    width: "auto",
  },
  style4: {
    marginInline: "auto",
    marginTop: {
      default: "3rem",
      "@media (width >= 48rem)": "4rem",
    },
    fontSize: {
      default: "3rem",
      "@media (width >= 48rem)": "4.5rem",
    },
    lineHeight: {
      default: ".98",
      "@media (width >= 48rem)": "1",
    },
    "--tw-leading": ".98",
    "--tw-font-weight": "600",
    fontWeight: "600",
    textWrap: "balance",
    position: {
      default: null,
      "@media (width >= 64rem)": "relative",
    },
    left: {
      default: null,
      "@media (width >= 64rem)": "50%",
    },
    width: {
      default: null,
      "@media (width >= 64rem)": "max-content",
    },
    maxWidth: {
      default: null,
      "@media (width >= 64rem)": "none",
    },
    "--tw-translate-x": {
      default: null,
      "@media (width >= 64rem)": "calc(calc(1 / 2 * 100%) * -1)",
    },
    translate: {
      default: null,
      "@media (width >= 64rem)": "calc(calc(1 / 2 * 100%) * -1) 0",
    },
    whiteSpace: {
      default: null,
      "@media (width >= 64rem)": "nowrap",
    },
  },
  style5: {
    display: {
      default: "block",
      "@media (width >= 64rem)": "inline",
    },
  },
  style6: {
    marginInline: "auto",
    marginTop: "1.5rem",
    fontSize: "1.125rem",
    lineHeight: "2rem",
    "--tw-leading": "2rem",
    color: "#4f4940",
  },
  style7: {
    marginTop: "2rem",
    display: "flex",
    flexWrap: "wrap",
    justifyContent: "center",
    columnGap: "1.25rem",
    rowGap: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
  },
  style8: {
    position: "relative",
    left: "50%",
    marginTop: "2.5rem",
    width: "100vw",
    maxWidth: "500px",
    "--tw-translate-x": "calc(calc(1 / 2 * 100%) * -1)",
    translate: "calc(calc(1 / 2 * 100%) * -1) 0",
    paddingInline: {
      default: "2rem",
      "@media (width >= 40rem)": "2.5rem",
    },
  },
  style9: {
    pointerEvents: "none",
    position: "absolute",
    top: "2.5rem",
    bottom: "6rem",
    left: {
      default: "2rem",
      "@media (width >= 40rem)": "2.5rem",
    },
    width: "3rem",
    borderRadius: "3.40282e38px",
    backgroundColor: "oklab(14.4788% 7.45058e-9 7.45058e-9 / .1)",
    "--tw-blur": "blur(40px)",
    filter: "blur(40px)        ",
  },
  style10: {
    pointerEvents: "none",
    position: "absolute",
    top: "2.5rem",
    right: {
      default: "2rem",
      "@media (width >= 40rem)": "2.5rem",
    },
    bottom: "6rem",
    width: "3rem",
    borderRadius: "3.40282e38px",
    backgroundColor: "oklab(14.4788% 7.45058e-9 7.45058e-9 / .1)",
    "--tw-blur": "blur(40px)",
    filter: "blur(40px)        ",
  },
  style11: {
    position: "relative",
    marginInline: "auto",
    maxWidth: "420px",
    overflow: "hidden",
    borderRadius: "1.5rem",
    borderInlineStyle: "solid",
    borderInlineWidth: "1px",
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    borderColor: "#e5e5e5",
    backgroundColor: "#fff",
    "--tw-shadow": "0 24px 70px #18161314",
    boxShadow:
      "0 0 #0000, 0 0 #0000, 0 0 #0000, 0 0 #0000, 0 24px 70px var(--tw-shadow-color, #18161314)",
    cornerShape: "squircle",
  },
  style12: {
    display: "flex",
    alignItems: "center",
    gap: ".5rem",
    paddingInline: "1rem",
    paddingBlock: ".75rem",
  },
  style13: {
    display: "flex",
    gap: ".5rem",
  },
  style14: {
    height: ".75rem",
    width: ".75rem",
    borderRadius: "3.40282e38px",
    backgroundColor: "#f87171",
  },
  style15: {
    height: ".75rem",
    width: ".75rem",
    borderRadius: "3.40282e38px",
    backgroundColor: "#facc15",
  },
  style16: {
    height: ".75rem",
    width: ".75rem",
    borderRadius: "3.40282e38px",
    backgroundColor: "#4ade80",
  },
  style17: {
    marginLeft: "auto",
    display: "flex",
    height: "1rem",
    width: "1.5rem",
    alignItems: "center",
    justifyContent: "flex-end",
  },
  style18: {
    color: "#737373",
  },
  style19: {
    position: "relative",
    minHeight: {
      default: "260px",
      "@media (width >= 40rem)": "300px",
    },
    overflow: "hidden",
    textAlign: "left",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
  },
  style20: {
    color: "#404040",
  },
  style21: {
    marginTop: "1rem",
    color: "#404040",
  },
  style22: {
    marginTop: "1rem",
    minHeight: "1.25rem",
    color: "#404040",
  },
  style23: {
    minHeight: "1.25rem",
    color: "#404040",
  },
  style24: {},
  style25: {
    listStyleType: "disc",
    paddingLeft: "1.25rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#404040",
  },
  style26: {
    height: "auto",
    width: "100%",
    borderRadius: ".75rem",
  },
  style27: {
    insetInline: "0",
    pointerEvents: "none",
    position: "absolute",
    bottom: "0",
    height: "7rem",
    "--tw-gradient-position": {
      default: "to top",
      "@supports (background-image: linear-gradient(in lab, red, red))":
        "to top in oklab",
    },
    backgroundImage: "linear-gradient(var(--tw-gradient-stops))",
    "--tw-gradient-from": "#fff",
    "--tw-gradient-stops":
      "var(--tw-gradient-position, #0000 0%, transparent 100%)",
    "--tw-gradient-to": "transparent",
  },
  style28: {
    position: "relative",
    display: "inline-flex",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    "--tw-font-weight": "500",
    fontWeight: "500",
  },
  style29: {
    display: "inline-flex",
    alignItems: "center",
    gap: ".375rem",
    borderTopLeftRadius: "3.40282e38px",
    borderBottomLeftRadius: "3.40282e38px",
    backgroundColor: "#181613",
    paddingBlock: ".75rem",
    paddingRight: ".5rem",
    paddingLeft: {
      default: "1rem",
      "@media (width >= 40rem)": "1.25rem",
    },
    fontSize: {
      default: "13px",
      "@media (width >= 40rem)": ".875rem",
    },
    color: "#fff",
    lineHeight: {
      default: null,
      "@media (width >= 40rem)": "1.25rem",
    },
  },
  style30: {
    display: "inline-flex",
    height: "100%",
    cursor: "pointer",
    alignItems: "center",
    borderTopRightRadius: "3.40282e38px",
    borderBottomRightRadius: "3.40282e38px",
    backgroundColor: "#181613",
    paddingBlock: ".75rem",
    paddingRight: ".75rem",
    paddingLeft: ".5rem",
    color: "#fff",
  },
  style31: {
    position: "absolute",
    top: "calc(100% + .5rem)",
    left: "0",
    zIndex: "10",
    width: "18rem",
    maxWidth: "calc(100vw - 2.5rem)",
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    padding: ".5rem",
    textAlign: "left",
    "--tw-shadow": "0 14px 40px #1816131f",
    boxShadow:
      "0 0 #0000, 0 0 #0000, 0 0 #0000, 0 0 #0000, 0 14px 40px var(--tw-shadow-color, #1816131f)",
  },
  style32: {
    display: "flex",
    alignItems: "center",
    gap: ".75rem",
    borderRadius: ".75rem",
    paddingInline: ".75rem",
    paddingBlock: ".625rem",
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, --tw-gradient-from, --tw-gradient-via, --tw-gradient-to",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
  style33: {
    marginLeft: "auto",
    borderRadius: "3.40282e38px",
    borderStyle: "solid",
    borderWidth: "1px",
    paddingInline: ".5rem",
    paddingBlock: ".125rem",
    fontSize: "11px",
    "--tw-leading": "1",
    lineHeight: "1",
    "--tw-font-weight": "500",
    fontWeight: "500",
    textTransform: "uppercase",
  },
  style34: {
    marginTop: ".25rem",
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    borderRadius: ".75rem",
    paddingInline: ".75rem",
    paddingBlock: ".625rem",
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, --tw-gradient-from, --tw-gradient-via, --tw-gradient-to",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
  style35: {
    flexShrink: "0",
  },
});
export function HeroSection() {
  return (
    <section {...stylex.props(styles.style1)}>
      <Link to="/" aria-label="Anarlog home" {...stylex.props(styles.style2)}>
        <AnarlogLogo sx={styles.style3} />
      </Link>
      <h1 {...stylex.props(styles.style4)}>
        <span {...stylex.props(styles.style5)}>The AI notepad for</span>{" "}
        <span {...stylex.props(styles.style5)}>private meetings.</span>
      </h1>
      <p {...stylex.props(styles.style6)}>
        Take bot-free, open-source meeting notes while keeping sensitive
        conversations secure and under your control.
      </p>
      <div {...stylex.props(styles.style7)}>
        <DownloadButton />
      </div>
      <HeroWorkflowDemo />
      <CredibilityLogoMarquee />
    </section>
  );
}
function HeroWorkflowDemo() {
  const [typedText1, setTypedText1] = useState("");
  const [typedText2, setTypedText2] = useState("");
  const [enhancedLines, setEnhancedLines] = useState(0);
  const [isTypingActive, setIsTypingActive] = useState(false);
  const text1 = "metrisc w/ john";
  const text2 = "stakehlder mtg";
  useMountEffect(() => {
    const timers = createTrackedTimers();
    const runAnimation = () => {
      setTypedText1("");
      setTypedText2("");
      setEnhancedLines(0);
      setIsTypingActive(false);
      let currentIndex1 = 0;
      timers.setTimeout(() => {
        setIsTypingActive(true);
        const interval1 = timers.setInterval(() => {
          if (currentIndex1 < text1.length) {
            setTypedText1(text1.slice(0, currentIndex1 + 1));
            currentIndex1++;
          } else {
            timers.clearInterval(interval1);
            let currentIndex2 = 0;
            const interval2 = timers.setInterval(() => {
              if (currentIndex2 < text2.length) {
                setTypedText2(text2.slice(0, currentIndex2 + 1));
                currentIndex2++;
              } else {
                timers.clearInterval(interval2);
                setIsTypingActive(false);
                timers.setTimeout(() => {
                  setEnhancedLines(1);
                  timers.setTimeout(() => {
                    setEnhancedLines(2);
                    timers.setTimeout(() => {
                      setEnhancedLines(3);
                      timers.setTimeout(() => {
                        setEnhancedLines(4);
                        timers.setTimeout(() => {
                          setEnhancedLines(5);
                          timers.setTimeout(() => {
                            setEnhancedLines(6);
                            timers.setTimeout(() => runAnimation(), 3000);
                          }, 800);
                        }, 800);
                      }, 800);
                    }, 800);
                  }, 800);
                }, 2000);
              }
            }, 50);
          }
        }, 50);
      }, 500);
    };
    const cancelIdle = runWhenIdle(runAnimation, {
      timeout: 2000,
      fallbackDelay: 1000,
    });
    return () => {
      cancelIdle();
      timers.clear();
    };
  });
  const isSummaryPhase = enhancedLines > 0;
  const isGeneratingSummary = enhancedLines > 0 && enhancedLines < 6;
  return (
    <div {...stylex.props(styles.style8)}>
      <div {...stylex.props(styles.style9)} aria-hidden="true" />
      <div {...stylex.props(styles.style10)} aria-hidden="true" />
      <div
        {...stylex.props(styles.style11)}
        style={{
          WebkitMaskImage:
            "linear-gradient(to bottom, black 0%, black calc(100% - 5rem), transparent 100%)",
          maskImage:
            "linear-gradient(to bottom, black 0%, black calc(100% - 5rem), transparent 100%)",
        }}
      >
        <div {...stylex.props(styles.style12)}>
          <div {...stylex.props(styles.style13)}>
            <div {...stylex.props(styles.style14)}></div>
            <div {...stylex.props(styles.style15)}></div>
            <div {...stylex.props(styles.style16)}></div>
          </div>
          <div {...stylex.props(styles.style17)}>
            {isGeneratingSummary ? (
              <Spinner size={12} {...stylex.props(styles.style18)} />
            ) : !isSummaryPhase ? (
              <DancingSticks
                amplitude={isTypingActive ? 1 : 0}
                height={12}
                color="#f87171"
              />
            ) : null}
          </div>
        </div>
        <div {...stylex.props(styles.style19)}>
          <div
            {...stylex.props([
              "absolute inset-0 space-y-3 px-5 pt-2 pb-5 transition-opacity duration-500 sm:px-6 sm:pt-3 sm:pb-6",
              isSummaryPhase ? "opacity-0" : "opacity-100",
            ])}
          >
            <div {...stylex.props(styles.style20)}>ui update - moble</div>
            <div {...stylex.props(styles.style20)}>api</div>
            <div {...stylex.props(styles.style21)}>new dash - urgnet</div>
            <div {...stylex.props(styles.style20)}>a/b tst next wk</div>
            <div {...stylex.props(styles.style22)}>
              {typedText1}
              <span
                {...stylex.props([
                  typedText1 && typedText1.length < text1.length
                    ? "animate-pulse"
                    : "opacity-0",
                ])}
              >
                |
              </span>
            </div>
            <div {...stylex.props(styles.style23)}>
              {typedText2}
              <span
                {...stylex.props([
                  typedText2 && typedText2.length < text2.length
                    ? "animate-pulse"
                    : "opacity-0",
                ])}
              >
                |
              </span>
            </div>
          </div>
          <div
            {...stylex.props([
              "absolute inset-0 space-y-4 overflow-hidden px-5 pt-2 pb-5 text-left transition-opacity duration-500 sm:px-6 sm:pt-3 sm:pb-6",
              isSummaryPhase ? "opacity-100" : "opacity-0",
            ])}
          >
            <div {...stylex.props(styles.style24)}>
              <h4
                {...stylex.props([
                  "font-semibold text-stone-700 transition-opacity duration-500",
                  enhancedLines >= 1 ? "opacity-100" : "opacity-0",
                ])}
              >
                Mobile UI Update and API Adjustments
              </h4>
              <ul {...stylex.props(styles.style25)}>
                <li
                  {...stylex.props([
                    "transition-opacity duration-500",
                    enhancedLines >= 1 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Sarah presented the new mobile UI update, which includes a
                  streamlined navigation bar and improved button placements for
                  better accessibility.
                </li>
                <li
                  {...stylex.props([
                    "transition-opacity duration-500",
                    enhancedLines >= 2 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Ben confirmed that API adjustments are needed to support
                  dynamic UI changes, particularly for fetching personalized
                  user data more efficiently.
                </li>
                <li
                  {...stylex.props([
                    "transition-opacity duration-500",
                    enhancedLines >= 3 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  The UI update will be implemented in phases, starting with
                  core navigation improvements. Ben will ensure API
                  modifications are completed before development begins.
                </li>
              </ul>
            </div>
            <div {...stylex.props(styles.style24)}>
              <h4
                {...stylex.props([
                  "font-semibold text-stone-700 transition-opacity duration-500",
                  enhancedLines >= 4 ? "opacity-100" : "opacity-0",
                ])}
              >
                New Dashboard - Urgent Priority
              </h4>
              <ul {...stylex.props(styles.style25)}>
                <li
                  {...stylex.props([
                    "transition-opacity duration-500",
                    enhancedLines >= 4 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Alice emphasized that the new analytics dashboard must be
                  prioritized due to increasing stakeholder demand.
                </li>
                <li
                  {...stylex.props([
                    "transition-opacity duration-500",
                    enhancedLines >= 5 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  The new dashboard will feature real-time user engagement
                  metrics and a customizable reporting system.
                </li>
                <li
                  {...stylex.props([
                    "transition-opacity duration-500",
                    enhancedLines >= 5 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Ben mentioned that backend infrastructure needs optimization
                  to handle real-time data processing.
                </li>
                <li
                  {...stylex.props([
                    "transition-opacity duration-500",
                    enhancedLines >= 5 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Mark stressed that the dashboard launch should align with
                  marketing efforts to maximize user adoption.
                </li>
                <li
                  {...stylex.props([
                    "transition-opacity duration-500",
                    enhancedLines >= 5 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Development will start immediately, and a basic prototype must
                  be ready for stakeholder review next week.
                </li>
              </ul>
            </div>
          </div>
        </div>
      </div>
      <div
        {...stylex.props([
          "pointer-events-none absolute right-1 bottom-9 z-10 w-[66%] transition-all duration-500 sm:-right-2 sm:bottom-12 sm:w-[68%]",
          isSummaryPhase
            ? "translate-y-2 opacity-0"
            : "translate-y-0 opacity-100",
        ])}
      >
        <img
          src={getResizedImageUrl("/images/hero-meeting-participants.webp", {
            width: 600,
          })}
          srcSet={[300, 600, 900]
            .map(
              (width) =>
                `${getResizedImageUrl(
                  "/images/hero-meeting-participants.webp",
                  {
                    width,
                  },
                )} ${width}w`,
            )
            .join(", ")}
          sizes="(min-width: 640px) 286px, 66vw"
          alt="Four participants in a video meeting"
          width={1200}
          height={215}
          {...stylex.props(styles.style26)}
          decoding="async"
        />
      </div>
      <div {...stylex.props(styles.style27)} aria-hidden="true" />
    </div>
  );
}
function DownloadButton() {
  const { track } = useAnalytics();
  const [open, setOpen] = useState(false);
  const [preferredPlatform, setPreferredPlatform] =
    useState<DesktopPlatform>("macos");
  const containerRef = useRef<HTMLDivElement>(null);
  const orderedSections = getOrderedDesktopDownloadSections(preferredPlatform);
  const preferredSection = orderedSections[0];
  const preferredDownload = preferredSection.downloads[0];
  const preferredLabel =
    preferredSection.platform === "macos"
      ? `Download for ${preferredDownload.name}`
      : `Download for ${preferredSection.name}`;
  useMountEffect(() => {
    setPreferredPlatform(detectDesktopPlatform(navigator.userAgent));
    const onPointerDown = (event: MouseEvent | TouchEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(event.target as Node)
      ) {
        setOpen(false);
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("touchstart", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("touchstart", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  });
  return (
    <div ref={containerRef} {...stylex.props(styles.style28)}>
      <a
        href={preferredDownload.url}
        onClick={() =>
          track("download_clicked", {
            platform: preferredSection.platform,
            spec: preferredDownload.name,
            source: "homepage",
          })
        }
        {...stylex.props(styles.style29)}
      >
        {getPlatformIcon(preferredSection.platform, 16)}
        <span>{preferredLabel}</span>
      </a>
      <button
        type="button"
        aria-label="Choose download platform"
        aria-expanded={open}
        aria-haspopup="menu"
        onClick={() => setOpen((previous) => !previous)}
        {...stylex.props(styles.style30)}
      >
        <CaretDown size={17} weight="bold" aria-hidden="true" />
      </button>
      {open && (
        <div role="menu" {...stylex.props(styles.style31)}>
          {orderedSections.map((section) =>
            section.downloads.map((download) => {
              if (!download.showInMenu) return null;
              if (download.url === preferredDownload.url) {
                return null;
              }
              return (
                <a
                  key={download.url}
                  href={download.url}
                  role="menuitem"
                  onClick={() => {
                    track("download_clicked", {
                      platform: section.platform,
                      spec: download.name,
                      source: "homepage_menu",
                    });
                    setOpen(false);
                  }}
                  {...stylex.props(styles.style32)}
                >
                  {getPlatformIcon(section.platform, 20)}
                  <span>
                    {getDownloadOptionLabel(section.platform, download.name)}
                  </span>
                  {section.status && (
                    <span {...stylex.props(styles.style33)}>
                      {section.status}
                    </span>
                  )}
                </a>
              );
            }),
          )}
          <Link
            to="/download/"
            role="menuitem"
            onClick={() => setOpen(false)}
            {...stylex.props(styles.style34)}
          >
            <span>View all downloads</span>
            <ArrowRight size={15} weight="bold" aria-hidden="true" />
          </Link>
        </div>
      )}
    </div>
  );
}
function getPlatformIcon(platform: DesktopPlatform, size: number) {
  const icon =
    platform === "windows"
      ? "simple-icons:windows11"
      : platform === "linux"
        ? "simple-icons:linux"
        : "simple-icons:apple";
  return (
    <Icon
      icon={icon}
      width={size}
      height={size}
      {...stylex.props(styles.style35)}
      aria-hidden="true"
    />
  );
}
function getDownloadOptionLabel(
  platform: DesktopPlatform,
  downloadName: string,
) {
  if (platform === "macos" && downloadName === "Intel") return "Apple Intel";
  const label = downloadName.replace(/ x64$/, "");
  if (platform === "linux") return `Linux ${label}`;
  return label;
}
