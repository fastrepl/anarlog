import { Icon } from "@iconify-icon/react";
import { ArrowRight, CaretDown } from "@phosphor-icons/react";
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
import { createTrackedTimers } from "@/lib/tracked-timers";

import { CredibilityLogoMarquee } from "./social-proof-sections";

export function HeroSection() {
  return (
    <section className="pt-10 pb-2 md:pt-12 md:pb-4">
      <Link to="/" aria-label="Anarlog home" className="inline-flex">
        <AnarlogLogo className="h-8 w-auto md:h-9" />
      </Link>
      <h1 className="font-hand mx-auto mt-12 max-w-3xl text-5xl leading-[0.98] font-semibold tracking-normal text-balance md:mt-16 md:text-7xl lg:relative lg:left-1/2 lg:w-max lg:max-w-none lg:-translate-x-1/2 lg:whitespace-nowrap">
        <span className="font-hand block lg:inline">The AI notepad for</span>{" "}
        <span className="font-hand block lg:inline">private meetings.</span>
      </h1>
      <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-[#4f4940]">
        Take bot-free, open-source meeting notes while keeping sensitive
        conversations secure and under your control.
      </p>
      <div className="mt-8 flex flex-wrap justify-center gap-x-5 gap-y-3 text-sm">
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
                }, 500);
              }
            }, 50);
          }
        }, 50);
      }, 500);
    };

    runAnimation();

    return () => {
      timers.clear();
    };
  });

  const isSummaryPhase = enhancedLines > 0;
  const isGeneratingSummary = enhancedLines > 0 && enhancedLines < 6;

  return (
    <div className="relative left-1/2 mt-10 w-screen max-w-[500px] -translate-x-1/2 px-8 sm:px-10">
      <div
        className="pointer-events-none absolute top-10 bottom-24 left-8 w-12 rounded-full bg-neutral-950/10 blur-2xl sm:left-10"
        aria-hidden="true"
      />
      <div
        className="pointer-events-none absolute top-10 right-8 bottom-24 w-12 rounded-full bg-neutral-950/10 blur-2xl sm:right-10"
        aria-hidden="true"
      />
      <div
        className="relative mx-auto max-w-[420px] overflow-hidden rounded-3xl border-x border-t border-neutral-200 bg-white shadow-[0_24px_70px_rgba(24,22,19,0.08)] [corner-shape:squircle]"
        style={{
          WebkitMaskImage:
            "linear-gradient(to bottom, black 0%, black calc(100% - 5rem), transparent 100%)",
          maskImage:
            "linear-gradient(to bottom, black 0%, black calc(100% - 5rem), transparent 100%)",
        }}
      >
        <div className="flex items-center gap-2 px-4 py-3">
          <div className="flex gap-2 [&>*]:[corner-shape:round]">
            <div className="h-3 w-3 rounded-full bg-red-400"></div>
            <div className="h-3 w-3 rounded-full bg-yellow-400"></div>
            <div className="h-3 w-3 rounded-full bg-green-400"></div>
          </div>
          <div className="ml-auto flex h-4 w-6 items-center justify-end">
            {isGeneratingSummary ? (
              <Spinner size={12} className="text-neutral-500" />
            ) : !isSummaryPhase ? (
              <DancingSticks
                amplitude={isTypingActive ? 1 : 0}
                height={12}
                color="#f87171"
              />
            ) : null}
          </div>
        </div>
        <div className="relative min-h-[260px] overflow-hidden text-left text-sm sm:min-h-[300px]">
          <div
            className={cn([
              "absolute inset-0 space-y-3 p-5 transition-opacity duration-500 sm:p-6",
              isSummaryPhase ? "opacity-0" : "opacity-100",
            ])}
          >
            <div className="text-neutral-700">ui update - moble</div>
            <div className="text-neutral-700">api</div>
            <div className="mt-4 text-neutral-700">new dash - urgnet</div>
            <div className="text-neutral-700">a/b tst next wk</div>
            <div className="mt-4 min-h-5 text-neutral-700">
              {typedText1}
              <span
                className={cn([
                  typedText1 && typedText1.length < text1.length
                    ? "animate-pulse"
                    : "opacity-0",
                ])}
              >
                |
              </span>
            </div>
            <div className="min-h-5 text-neutral-700">
              {typedText2}
              <span
                className={cn([
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
            className={cn([
              "absolute inset-0 space-y-4 overflow-hidden p-5 text-left transition-opacity duration-500 sm:p-6",
              isSummaryPhase ? "opacity-100" : "opacity-0",
            ])}
          >
            <div className="space-y-2">
              <h4
                className={cn([
                  "font-semibold text-stone-700 transition-opacity duration-500",
                  enhancedLines >= 1 ? "opacity-100" : "opacity-0",
                ])}
              >
                Mobile UI Update and API Adjustments
              </h4>
              <ul className="list-disc space-y-2 pl-5 text-sm text-neutral-700">
                <li
                  className={cn([
                    "transition-opacity duration-500",
                    enhancedLines >= 1 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Sarah presented the new mobile UI update, which includes a
                  streamlined navigation bar and improved button placements for
                  better accessibility.
                </li>
                <li
                  className={cn([
                    "transition-opacity duration-500",
                    enhancedLines >= 2 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Ben confirmed that API adjustments are needed to support
                  dynamic UI changes, particularly for fetching personalized
                  user data more efficiently.
                </li>
                <li
                  className={cn([
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
            <div className="space-y-2">
              <h4
                className={cn([
                  "font-semibold text-stone-700 transition-opacity duration-500",
                  enhancedLines >= 4 ? "opacity-100" : "opacity-0",
                ])}
              >
                New Dashboard - Urgent Priority
              </h4>
              <ul className="list-disc space-y-2 pl-5 text-sm text-neutral-700">
                <li
                  className={cn([
                    "transition-opacity duration-500",
                    enhancedLines >= 4 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Alice emphasized that the new analytics dashboard must be
                  prioritized due to increasing stakeholder demand.
                </li>
                <li
                  className={cn([
                    "transition-opacity duration-500",
                    enhancedLines >= 5 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  The new dashboard will feature real-time user engagement
                  metrics and a customizable reporting system.
                </li>
                <li
                  className={cn([
                    "transition-opacity duration-500",
                    enhancedLines >= 5 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Ben mentioned that backend infrastructure needs optimization
                  to handle real-time data processing.
                </li>
                <li
                  className={cn([
                    "transition-opacity duration-500",
                    enhancedLines >= 5 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Mark stressed that the dashboard launch should align with
                  marketing efforts to maximize user adoption.
                </li>
                <li
                  className={cn([
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
        className="pointer-events-none absolute right-0 bottom-0 left-0 h-28 bg-linear-to-t from-white to-transparent"
        aria-hidden="true"
      />
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
    <div
      ref={containerRef}
      className="relative inline-flex text-sm font-medium"
    >
      <a
        href={preferredDownload.url}
        onClick={() =>
          track("download_clicked", {
            platform: preferredSection.platform,
            spec: preferredDownload.name,
            source: "homepage",
          })
        }
        className="inline-flex items-center gap-1.5 rounded-l-full bg-[#181613] py-3 pr-2 pl-4 text-[13px] text-white sm:pl-5 sm:text-sm"
      >
        <Icon
          icon={getPlatformIcon(preferredSection.platform)}
          width={16}
          height={16}
          className="shrink-0"
          aria-hidden="true"
        />
        <span>{preferredLabel}</span>
      </a>
      <button
        type="button"
        aria-label="Choose download platform"
        aria-expanded={open}
        aria-haspopup="menu"
        onClick={() => setOpen((previous) => !previous)}
        className="inline-flex h-full cursor-pointer items-center rounded-r-full bg-[#181613] py-3 pr-3 pl-2 text-white"
      >
        <CaretDown size={17} weight="bold" aria-hidden="true" />
      </button>
      {open && (
        <div
          role="menu"
          className="surface border-color-brand absolute top-[calc(100%+0.5rem)] left-0 w-72 max-w-[calc(100vw-2.5rem)] rounded-2xl border p-2 text-left shadow-[0_14px_40px_rgba(24,22,19,0.12)]"
        >
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
                  className="text-color hover:surface-subtle flex items-center gap-3 rounded-xl px-3 py-2.5 transition-colors"
                >
                  <Icon
                    icon={getPlatformIcon(section.platform)}
                    width={20}
                    height={20}
                    className="shrink-0"
                    aria-hidden="true"
                  />
                  <span>
                    {getDownloadOptionLabel(section.platform, download.name)}
                  </span>
                  {section.status && (
                    <span className="border-color-subtle text-color-muted ml-auto rounded-full border px-2 py-0.5 text-[11px] leading-none font-medium tracking-wide uppercase">
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
            className="text-color-muted hover:surface-subtle mt-1 flex items-center justify-between rounded-xl px-3 py-2.5 transition-colors"
          >
            <span>View all downloads</span>
            <ArrowRight size={15} weight="bold" aria-hidden="true" />
          </Link>
        </div>
      )}
    </div>
  );
}

function getPlatformIcon(platform: DesktopPlatform) {
  if (platform === "windows") return "simple-icons:windows11";
  if (platform === "linux") return "simple-icons:linux";
  return "simple-icons:apple";
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
