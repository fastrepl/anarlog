import { Icon } from "@iconify-icon/react";
import { ArrowRight, CaretDown } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { useRef, useState } from "react";

import { colors, fonts, media, radii } from "@anlg/design-system/tokens.stylex";
import { DancingSticks } from "@anlg/ui/components/ui/dancing-sticks";
import { Spinner } from "@anlg/ui/components/ui/spinner";

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

const cursorPulse = stylex.keyframes({
  "0%, 100%": { opacity: 1 },
  "50%": { opacity: 0.5 },
});

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
      default: 0.98,
      "@media (width >= 48rem)": 1,
    },
    fontFamily: fonts.hand,
    fontWeight: 600,
    letterSpacing: 0,
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
      default: "48rem",
      "@media (width >= 64rem)": "none",
    },
    transform: {
      default: null,
      "@media (width >= 64rem)": "translateX(-50%)",
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
    fontFamily: fonts.hand,
    letterSpacing: 0,
  },
  style6: {
    marginInline: "auto",
    marginTop: "1.5rem",
    maxWidth: "42rem",
    fontSize: "1.125rem",
    lineHeight: "2rem",
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
    transform: "translateX(-50%)",
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
    borderRadius: radii.full,
    backgroundColor: "oklab(14.4788% 7.45058e-9 7.45058e-9 / .1)",
    filter: "blur(40px)",
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
    borderRadius: radii.full,
    backgroundColor: "oklab(14.4788% 7.45058e-9 7.45058e-9 / .1)",
    filter: "blur(40px)",
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
    backgroundColor: colors.card,
    boxShadow: "0 24px 70px rgb(24 22 19 / 0.08)",
    cornerShape: "squircle",
    WebkitMaskImage:
      "linear-gradient(to bottom, black 0%, black calc(100% - 5rem), transparent 100%)",
    maskImage:
      "linear-gradient(to bottom, black 0%, black calc(100% - 5rem), transparent 100%)",
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
    borderRadius: radii.full,
    backgroundColor: "#f87171",
    cornerShape: "round",
  },
  style15: {
    height: ".75rem",
    width: ".75rem",
    borderRadius: radii.full,
    backgroundColor: "#facc15",
    cornerShape: "round",
  },
  style16: {
    height: ".75rem",
    width: ".75rem",
    borderRadius: radii.full,
    backgroundColor: "#4ade80",
    cornerShape: "round",
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
  workflowContent: {
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
  draftNote: {
    color: "#404040",
  },
  draftNoteSectionStart: {
    marginTop: ".25rem",
    color: "#404040",
  },
  draftTypedNoteSectionStart: {
    marginTop: ".25rem",
    minHeight: "1.25rem",
    color: "#404040",
  },
  draftTypedNote: {
    minHeight: "1.25rem",
    color: "#404040",
  },
  summarySection: {
    display: "flex",
    flexDirection: "column",
    gap: ".5rem",
  },
  summaryList: {
    listStyleType: "disc",
    paddingLeft: "1.25rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#404040",
  },
  participantImage: {
    height: "auto",
    width: "100%",
    borderRadius: ".75rem",
  },
  workflowBottomFade: {
    insetInline: 0,
    pointerEvents: "none",
    position: "absolute",
    bottom: 0,
    height: "7rem",
    backgroundImage: `linear-gradient(to top, ${colors.card}, transparent)`,
  },
  style28: {
    position: "relative",
    display: "inline-flex",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
  },
  style29: {
    display: "inline-flex",
    alignItems: "center",
    gap: ".375rem",
    borderTopLeftRadius: radii.full,
    borderBottomLeftRadius: radii.full,
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
    borderTopRightRadius: radii.full,
    borderBottomRightRadius: radii.full,
    backgroundColor: "#181613",
    paddingBlock: ".75rem",
    paddingRight: ".75rem",
    paddingLeft: ".5rem",
    color: "#fff",
  },
  style31: {
    position: "absolute",
    top: "calc(100% + .5rem)",
    left: 0,
    zIndex: 10,
    width: "18rem",
    maxWidth: "calc(100vw - 2.5rem)",
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: colors.border,
    backgroundColor: colors.card,
    padding: ".5rem",
    textAlign: "left",
    boxShadow: "0 14px 40px rgb(24 22 19 / 0.12)",
  },
  style32: {
    display: "flex",
    alignItems: "center",
    gap: ".75rem",
    borderRadius: ".75rem",
    paddingInline: ".75rem",
    paddingBlock: ".625rem",
    transitionProperty: "color, background-color, border-color",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.muted,
    },
    color: colors.foreground,
  },
  style33: {
    marginLeft: "auto",
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: colors.border,
    paddingInline: ".5rem",
    paddingBlock: ".125rem",
    fontSize: "11px",
    lineHeight: 1,
    fontWeight: 500,
    letterSpacing: ".025em",
    textTransform: "uppercase",
    color: colors.mutedForeground,
  },
  style34: {
    marginTop: ".25rem",
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    borderRadius: ".75rem",
    paddingInline: ".75rem",
    paddingBlock: ".625rem",
    transitionProperty: "color, background-color, border-color",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.muted,
    },
    color: colors.mutedForeground,
  },
  style35: {
    flexShrink: 0,
  },
  draftNotesLayer: {
    display: "flex",
    flexDirection: "column",
    gap: ".75rem",
    inset: 0,
    paddingInline: {
      default: "1.25rem",
      [media.sm]: "1.5rem",
    },
    paddingTop: {
      default: ".5rem",
      [media.sm]: ".75rem",
    },
    paddingBottom: {
      default: "1.25rem",
      [media.sm]: "1.5rem",
    },
    position: "absolute",
    transitionDuration: {
      default: "500ms",
      [media.reducedMotion]: "0ms",
    },
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
  },
  summaryNotesLayer: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
    inset: 0,
    overflow: "hidden",
    paddingInline: {
      default: "1.25rem",
      [media.sm]: "1.5rem",
    },
    paddingTop: {
      default: ".5rem",
      [media.sm]: ".75rem",
    },
    paddingBottom: {
      default: "1.25rem",
      [media.sm]: "1.5rem",
    },
    position: "absolute",
    textAlign: "left",
    transitionDuration: {
      default: "500ms",
      [media.reducedMotion]: "0ms",
    },
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
  },
  contentVisible: {
    opacity: 1,
  },
  contentHidden: {
    opacity: 0,
  },
  typingCursor: {
    animationDuration: "2s",
    animationIterationCount: "infinite",
    animationName: {
      default: cursorPulse,
      [media.reducedMotion]: "none",
    },
    animationTimingFunction: "cubic-bezier(.4, 0, .6, 1)",
  },
  summaryHeading: {
    color: "#44403c",
    fontWeight: 600,
    transitionDuration: {
      default: "500ms",
      [media.reducedMotion]: "0ms",
    },
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
  },
  summaryListItem: {
    marginTop: {
      default: ".5rem",
      ":first-child": 0,
    },
    transitionDuration: {
      default: "500ms",
      [media.reducedMotion]: "0ms",
    },
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
  },
  participantOverlay: {
    bottom: {
      default: "2.25rem",
      [media.sm]: "3rem",
    },
    pointerEvents: "none",
    position: "absolute",
    right: {
      default: ".25rem",
      [media.sm]: "-.5rem",
    },
    transitionDuration: {
      default: "500ms",
      [media.reducedMotion]: "0ms",
    },
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    width: {
      default: "66%",
      [media.sm]: "68%",
    },
    zIndex: 10,
  },
  participantOverlayVisible: {
    opacity: 1,
    transform: {
      default: "translateY(0)",
      [media.reducedMotion]: "none",
    },
  },
  participantOverlayHidden: {
    opacity: 0,
    transform: {
      default: "translateY(.5rem)",
      [media.reducedMotion]: "none",
    },
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
      <div {...stylex.props(styles.style11)}>
        <div {...stylex.props(styles.style12)}>
          <div {...stylex.props(styles.style13)}>
            <div {...stylex.props(styles.style14)}></div>
            <div {...stylex.props(styles.style15)}></div>
            <div {...stylex.props(styles.style16)}></div>
          </div>
          <div {...stylex.props(styles.style17)}>
            {isGeneratingSummary ? (
              <Spinner size={12} sx={styles.style18} />
            ) : !isSummaryPhase ? (
              <DancingSticks
                amplitude={isTypingActive ? 1 : 0}
                height={12}
                color="#f87171"
              />
            ) : null}
          </div>
        </div>
        <div {...stylex.props(styles.workflowContent)}>
          <div
            {...stylex.props(
              styles.draftNotesLayer,
              isSummaryPhase ? styles.contentHidden : styles.contentVisible,
            )}
          >
            <div {...stylex.props(styles.draftNote)}>ui update - moble</div>
            <div {...stylex.props(styles.draftNote)}>api</div>
            <div {...stylex.props(styles.draftNoteSectionStart)}>
              new dash - urgnet
            </div>
            <div {...stylex.props(styles.draftNote)}>a/b tst next wk</div>
            <div {...stylex.props(styles.draftTypedNoteSectionStart)}>
              {typedText1}
              <span
                {...stylex.props(
                  typedText1 && typedText1.length < text1.length
                    ? styles.typingCursor
                    : styles.contentHidden,
                )}
              >
                |
              </span>
            </div>
            <div {...stylex.props(styles.draftTypedNote)}>
              {typedText2}
              <span
                {...stylex.props(
                  typedText2 && typedText2.length < text2.length
                    ? styles.typingCursor
                    : styles.contentHidden,
                )}
              >
                |
              </span>
            </div>
          </div>
          <div
            {...stylex.props(
              styles.summaryNotesLayer,
              isSummaryPhase ? styles.contentVisible : styles.contentHidden,
            )}
          >
            <div {...stylex.props(styles.summarySection)}>
              <h4
                {...stylex.props(
                  styles.summaryHeading,
                  enhancedLines >= 1
                    ? styles.contentVisible
                    : styles.contentHidden,
                )}
              >
                Mobile UI Update and API Adjustments
              </h4>
              <ul {...stylex.props(styles.summaryList)}>
                <li
                  {...stylex.props(
                    styles.summaryListItem,
                    enhancedLines >= 1
                      ? styles.contentVisible
                      : styles.contentHidden,
                  )}
                >
                  Sarah presented the new mobile UI update, which includes a
                  streamlined navigation bar and improved button placements for
                  better accessibility.
                </li>
                <li
                  {...stylex.props(
                    styles.summaryListItem,
                    enhancedLines >= 2
                      ? styles.contentVisible
                      : styles.contentHidden,
                  )}
                >
                  Ben confirmed that API adjustments are needed to support
                  dynamic UI changes, particularly for fetching personalized
                  user data more efficiently.
                </li>
                <li
                  {...stylex.props(
                    styles.summaryListItem,
                    enhancedLines >= 3
                      ? styles.contentVisible
                      : styles.contentHidden,
                  )}
                >
                  The UI update will be implemented in phases, starting with
                  core navigation improvements. Ben will ensure API
                  modifications are completed before development begins.
                </li>
              </ul>
            </div>
            <div {...stylex.props(styles.summarySection)}>
              <h4
                {...stylex.props(
                  styles.summaryHeading,
                  enhancedLines >= 4
                    ? styles.contentVisible
                    : styles.contentHidden,
                )}
              >
                New Dashboard - Urgent Priority
              </h4>
              <ul {...stylex.props(styles.summaryList)}>
                <li
                  {...stylex.props(
                    styles.summaryListItem,
                    enhancedLines >= 4
                      ? styles.contentVisible
                      : styles.contentHidden,
                  )}
                >
                  Alice emphasized that the new analytics dashboard must be
                  prioritized due to increasing stakeholder demand.
                </li>
                <li
                  {...stylex.props(
                    styles.summaryListItem,
                    enhancedLines >= 5
                      ? styles.contentVisible
                      : styles.contentHidden,
                  )}
                >
                  The new dashboard will feature real-time user engagement
                  metrics and a customizable reporting system.
                </li>
                <li
                  {...stylex.props(
                    styles.summaryListItem,
                    enhancedLines >= 5
                      ? styles.contentVisible
                      : styles.contentHidden,
                  )}
                >
                  Ben mentioned that backend infrastructure needs optimization
                  to handle real-time data processing.
                </li>
                <li
                  {...stylex.props(
                    styles.summaryListItem,
                    enhancedLines >= 5
                      ? styles.contentVisible
                      : styles.contentHidden,
                  )}
                >
                  Mark stressed that the dashboard launch should align with
                  marketing efforts to maximize user adoption.
                </li>
                <li
                  {...stylex.props(
                    styles.summaryListItem,
                    enhancedLines >= 5
                      ? styles.contentVisible
                      : styles.contentHidden,
                  )}
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
        {...stylex.props(
          styles.participantOverlay,
          isSummaryPhase
            ? styles.participantOverlayHidden
            : styles.participantOverlayVisible,
        )}
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
          {...stylex.props(styles.participantImage)}
          decoding="async"
        />
      </div>
      <div {...stylex.props(styles.workflowBottomFade)} aria-hidden="true" />
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
