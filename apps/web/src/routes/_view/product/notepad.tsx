import { createFileRoute, Link } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { cn } from "@hypr/utils";

import { MockWindow } from "@/components/mock-window";
import { usePlatform } from "@/hooks/use-platform";

export const Route = createFileRoute("/_view/product/notepad")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Notepad - Char" },
      {
        name: "description",
        content:
          "Private, local-first notepad with no bots. Record meetings directly from your device with AI processing that never leaves your computer.",
      },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
});

function Component() {
  const platform = usePlatform();

  const getPrimaryCTA = () => {
    if (platform === "mac") {
      return {
        labelFull: "Download for Mac",
        labelShort: "Download",
        href: "/download/apple-silicon",
        isDownload: true,
      };
    }
    return {
      labelFull: platform === "mobile" ? "Remind Me" : "Join Waitlist",
      labelShort: platform === "mobile" ? "Remind Me" : "Join Waitlist",
      href: "/",
      isDownload: false,
    };
  };

  const primaryCTA = getPrimaryCTA();

  return (
    <div
      className="overflow-hidden bg-linear-to-b from-white via-stone-50/20 to-white sm:h-[calc(100vh-65px)]"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="relative mx-auto flex h-full max-w-6xl flex-col border-x border-neutral-100 bg-white">
        <div className="relative z-10 flex flex-1 items-center justify-center bg-[linear-gradient(to_bottom,rgba(245,245,244,0.2),white_50%,rgba(245,245,244,0.3))] px-6 py-12">
          <div className="pointer-events-auto mx-auto max-w-4xl text-center">
            <h1 className="mx-auto mb-6 max-w-2xl font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
              Your private notepad. <br className="hidden sm:inline" />
              No bots. Local-first.
            </h1>
            <p className="mx-auto mb-8 max-w-2xl text-lg text-neutral-600 sm:text-xl">
              Char takes your memos and transcripts to make crazy good notes
            </p>

            <AnimatedNotesDemo />

            <div className="flex flex-row items-center justify-center gap-4">
              {primaryCTA.isDownload ? (
                <a
                  href={primaryCTA.href}
                  download
                  className={cn([
                    "inline-block rounded-full px-8 py-3 text-base font-medium",
                    "bg-linear-to-t from-stone-600 to-stone-500 text-white",
                    "transition-transform hover:scale-105 active:scale-95",
                  ])}
                >
                  <span className="hidden sm:inline">
                    {primaryCTA.labelFull}
                  </span>
                  <span className="sm:hidden">{primaryCTA.labelShort}</span>
                </a>
              ) : (
                <Link
                  to={primaryCTA.href}
                  className={cn([
                    "inline-block rounded-full px-8 py-3 text-base font-medium",
                    "bg-linear-to-t from-stone-600 to-stone-500 text-white",
                    "transition-transform hover:scale-105 active:scale-95",
                  ])}
                >
                  <span className="hidden sm:inline">
                    {primaryCTA.labelFull}
                  </span>
                  <span className="sm:hidden">{primaryCTA.labelShort}</span>
                </Link>
              )}
              <Link
                to="/product/ai-notetaking/"
                className={cn([
                  "inline-block rounded-full px-8 py-3 text-base font-medium",
                  "rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 shadow-xs hover:scale-[102%] hover:shadow-md active:scale-[98%]",
                  "transition-all",
                ])}
              >
                Learn More
              </Link>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function AnimatedNotesDemo() {
  const [typedText1, setTypedText1] = useState("");
  const [typedText2, setTypedText2] = useState("");
  const [showEnhanced, setShowEnhanced] = useState(false);
  const [enhancedLines, setEnhancedLines] = useState(0);

  const text1 = "metrisc w/ john";
  const text2 = "stakehlder mtg";

  useEffect(() => {
    const runAnimation = () => {
      setTypedText1("");
      setTypedText2("");
      setShowEnhanced(false);
      setEnhancedLines(0);

      let currentIndex1 = 0;
      setTimeout(() => {
        const interval1 = setInterval(() => {
          if (currentIndex1 < text1.length) {
            setTypedText1(text1.slice(0, currentIndex1 + 1));
            currentIndex1++;
          } else {
            clearInterval(interval1);

            let currentIndex2 = 0;
            const interval2 = setInterval(() => {
              if (currentIndex2 < text2.length) {
                setTypedText2(text2.slice(0, currentIndex2 + 1));
                currentIndex2++;
              } else {
                clearInterval(interval2);
                setTimeout(() => {
                  setShowEnhanced(true);
                  setTimeout(() => {
                    setEnhancedLines(1);
                    setTimeout(() => {
                      setEnhancedLines(2);
                      setTimeout(() => {
                        setEnhancedLines(3);
                        setTimeout(() => {
                          setEnhancedLines(4);
                          setTimeout(() => {
                            setEnhancedLines(5);
                            setTimeout(() => {
                              setEnhancedLines(6);
                              setTimeout(() => runAnimation(), 2000);
                            }, 800);
                          }, 800);
                        }, 800);
                      }, 800);
                    }, 800);
                  }, 500);
                }, 1000);
              }
            }, 50);
          }
        }, 50);
      }, 500);
    };

    runAnimation();
  }, []);

  if (showEnhanced) {
    return (
      <MockWindow
        variant="desktop"
        className="mx-auto mb-8 w-full rounded-lg border-b sm:w-[420px] md:w-[480px] lg:w-[540px]"
      >
        <div className="flex h-96 flex-col gap-4 overflow-hidden p-6 text-left">
          <div className="flex flex-col gap-2">
            <h4
              className={cn([
                "text-lg font-semibold text-stone-700 transition-opacity duration-500",
                enhancedLines >= 1 ? "opacity-100" : "opacity-0",
              ])}
            >
              Mobile UI Update and API Adjustments
            </h4>
            <ul className="flex list-disc flex-col gap-2 pl-5 text-neutral-700">
              <li
                className={cn([
                  "transition-opacity duration-500",
                  enhancedLines >= 2 ? "opacity-100" : "opacity-0",
                ])}
              >
                Sarah presented the new mobile UI update, which includes a
                streamlined navigation bar and improved button placements for
                better accessibility.
              </li>
              <li
                className={cn([
                  "transition-opacity duration-500",
                  enhancedLines >= 3 ? "opacity-100" : "opacity-0",
                ])}
              >
                Ben confirmed that API adjustments are needed to support dynamic
                UI changes, particularly for fetching personalized user data
                more efficiently.
              </li>
              <li
                className={cn([
                  "transition-opacity duration-500",
                  enhancedLines >= 4 ? "opacity-100" : "opacity-0",
                ])}
              >
                The UI update will be implemented in phases, starting with core
                navigation improvements. Ben will ensure API modifications are
                completed before development begins.
              </li>
            </ul>
          </div>
          <div className="flex flex-col gap-2">
            <h4
              className={cn([
                "text-lg font-semibold text-stone-700 transition-opacity duration-500",
                enhancedLines >= 5 ? "opacity-100" : "opacity-0",
              ])}
            >
              New Dashboard – Urgent Priority
            </h4>
            <ul className="flex list-disc flex-col gap-2 pl-5 text-neutral-700">
              <li
                className={cn([
                  "transition-opacity duration-500",
                  enhancedLines >= 6 ? "opacity-100" : "opacity-0",
                ])}
              >
                Alice emphasized that the new analytics dashboard must be
                prioritized due to increasing stakeholder demand.
              </li>
            </ul>
          </div>
        </div>
      </MockWindow>
    );
  }

  return (
    <MockWindow
      showAudioIndicator
      variant="desktop"
      className="mx-auto mb-8 w-full rounded-lg border-b sm:w-[420px] md:w-[480px] lg:w-[540px]"
    >
      <div className="h-96 overflow-hidden p-6 text-left">
        <div className="text-neutral-700">ui update - mobile</div>
        <div className="text-neutral-700">api</div>
        <div className="mt-4 text-neutral-700">new dash - urgent</div>
        <div className="text-neutral-700">a/b test next wk</div>
        <div className="mt-4 text-neutral-700">
          {typedText1}
          {typedText1 && typedText1.length < text1.length && (
            <span className="animate-pulse">|</span>
          )}
        </div>
        <div className="text-neutral-700">
          {typedText2}
          {typedText2 && typedText2.length < text2.length && (
            <span className="animate-pulse">|</span>
          )}
        </div>
      </div>
    </MockWindow>
  );
}
