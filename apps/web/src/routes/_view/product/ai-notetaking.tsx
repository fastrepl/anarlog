import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { CheckIcon, ChevronDownIcon, SearchIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { memo, useCallback, useEffect, useRef, useState } from "react";

import { Typewriter } from "@hypr/ui/components/ui/typewriter";
import { cn } from "@hypr/utils";

import { Image } from "@/components/image";
import { MockWindow } from "@/components/mock-window";
import { SlashSeparator } from "@/components/slash-separator";

export const Route = createFileRoute("/_view/product/ai-notetaking")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Notetaking - Char" },
      {
        name: "description",
        content:
          "Complete AI-powered notetaking solution. Record meetings, transcribe audio, and get intelligent summaries with customizable templates. Works with any video conferencing tool.",
      },
      { name: "robots", content: "noindex, nofollow" },
      { property: "og:title", content: "AI Notetaking - Char" },
      {
        property: "og:description",
        content:
          "Record meetings in real-time or upload audio files. Get instant AI transcriptions, summaries, and action items with customizable templates.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/product/ai-notetaking",
      },
    ],
  }),
});

const tabs = [
  {
    title: "Compact Mode",
    description:
      "The default collapsed overlay that indicates the meeting is being listened to. Minimal and unobtrusive, staying out of your way.",
    image: "/api/images/hyprnote/float-compact.jpg",
  },
  {
    title: "Memos",
    description:
      "Take quick notes during the meeting. Jot down important points, ideas, or reminders without losing focus on the conversation.",
    image: "/api/images/hyprnote/float-memos.jpg",
  },
  {
    title: "Transcript",
    description:
      "Watch the live transcript as the conversation unfolds in real-time, so you never miss what was said during the meeting.",
    image: "/api/images/hyprnote/float-transcript.jpg",
  },
  {
    title: "Live Insights",
    description:
      "Get a rolling summary of the past 5 minutes with AI-powered suggestions. For sales calls, receive prompts for qualification questions and next steps.",
    image: "/api/images/hyprnote/float-insights.jpg",
  },
  {
    title: "Chat",
    description:
      "Ask questions and get instant answers during the meeting. Query the transcript, get clarifications, or find specific information on the fly.",
    image: "/api/images/hyprnote/float-chat.jpg",
  },
];

function Component() {
  return (
    <div
      className="min-h-screen overflow-x-hidden bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <HeroSection />
        <SlashSeparator />
        <EditorSection />
        <SlashSeparator />
        <TranscriptionSection />
        <SlashSeparator />
        <SummariesSection />
        <SlashSeparator />
        <SearchSection />
        <SlashSeparator />
        <SharingSection />
        <SlashSeparator />
        <FloatingPanelSection />
        <SlashSeparator />
        <CTASection />
      </div>
    </div>
  );
}

function HeroSection() {
  return (
    <div className="bg-linear-to-b from-stone-50/30 to-stone-100/30">
      <div className="px-6 py-12 lg:py-20">
        <header className="mx-auto mb-12 max-w-4xl text-center">
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            AI Notepad for Smarter Meeting Notes
          </h1>
          <p className="text-lg text-neutral-600 sm:text-xl">
            You focus on the conversation. AI transcribes, summarizes,
            <br className="hidden sm:inline" /> and fills in what you missed.
          </p>
          <div className="mt-8">
            <Link
              to="/download/"
              className={cn([
                "inline-block rounded-full px-8 py-3 text-base font-medium",
                "bg-linear-to-t from-stone-600 to-stone-500 text-white",
                "transition-transform hover:scale-105 active:scale-95",
              ])}
            >
              Download for free
            </Link>
          </div>
        </header>
      </div>
      <div className="relative aspect-video w-full overflow-hidden border-t border-neutral-100">
        <img
          src="/api/images/hyprnote/ai-notetaking-hero.jpg"
          alt="AI notetaking in action"
          className="h-full w-full object-cover"
        />
      </div>
    </div>
  );
}

function EditorSection() {
  return (
    <section id="editor" className="bg-stone-50/30">
      <div className="hidden sm:grid sm:grid-cols-2">
        <div className="flex items-center p-8">
          <div className="flex flex-col gap-4">
            <h2 className="font-serif text-3xl text-stone-700">
              Simple, Familiar Notepad
            </h2>
            <p className="text-base leading-relaxed text-neutral-600">
              Char comes with a easy-to-use text editor where you can jot down
              stuff in markdown.
            </p>
            <ul className="flex flex-col gap-3">
              <li className="flex items-start gap-3">
                <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
                <span className="text-neutral-600">
                  Full markdown syntax support for quick formatting
                </span>
              </li>
              <li className="flex items-start gap-3">
                <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
                <span className="text-neutral-600">
                  Clean, distraction-free writing experience
                </span>
              </li>
              <li className="flex items-start gap-3">
                <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
                <span className="text-neutral-600">
                  Rich text editing with familiar keyboard shortcuts
                </span>
              </li>
            </ul>
          </div>
        </div>
        <div className="flex items-end justify-center overflow-hidden bg-stone-50/30 px-8 pt-8 pb-0">
          <MockWindow>
            <div className="h-80 overflow-hidden p-6">
              <AnimatedMarkdownDemo />
            </div>
          </MockWindow>
        </div>
      </div>

      <div className="sm:hidden">
        <div className="border-b border-neutral-100 p-6">
          <h2 className="mb-3 font-serif text-2xl text-stone-700">
            Simple, Familiar Notepad
          </h2>
          <p className="mb-4 text-sm leading-relaxed text-neutral-600">
            Char comes with a easy-to-use text editor where you can jot down
            stuff in markdown.
          </p>
          <ul className="flex flex-col gap-3">
            <li className="flex items-start gap-3">
              <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
              <span className="text-sm text-neutral-600">
                Full markdown syntax support for quick formatting
              </span>
            </li>
            <li className="flex items-start gap-3">
              <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
              <span className="text-sm text-neutral-600">
                Clean, distraction-free writing experience
              </span>
            </li>
            <li className="flex items-start gap-3">
              <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
              <span className="text-sm text-neutral-600">
                Rich text editing with familiar keyboard shortcuts
              </span>
            </li>
          </ul>
        </div>
        <div className="overflow-clip bg-stone-50/30 px-6 pb-0">
          <MockWindow variant="mobile">
            <div className="h-[200px] overflow-hidden p-6">
              <AnimatedMarkdownDemo isMobile />
            </div>
          </MockWindow>
        </div>
      </div>
    </section>
  );
}

function AudioTranscriptionDemo() {
  const [progress, setProgress] = useState(0);

  const words = [
    { position: 0.02, text: "Welcome" },
    { position: 0.15, text: "to" },
    { position: 0.2, text: "today's" },
    { position: 0.33, text: "meeting" },
    { position: 0.48, text: "Let's" },
    { position: 0.59, text: "discuss" },
    { position: 0.73, text: "the" },
    { position: 0.79, text: "Q4" },
    { position: 0.86, text: "roadmap" },
  ];

  const audioBarHeights = useState(() => {
    const audioBarCount = 60;
    return Array.from({ length: audioBarCount }, () => {
      return Math.random() * 0.8 + 0.2;
    });
  })[0];

  useEffect(() => {
    const duration = 8000;
    const startTime = Date.now();

    const animate = () => {
      const elapsed = Date.now() - startTime;
      const newProgress = (elapsed % duration) / duration;

      setProgress(newProgress);

      requestAnimationFrame(animate);
    };

    const animationId = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(animationId);
  }, []);

  return (
    <div
      className="relative flex w-full flex-col items-center justify-center gap-6 bg-white p-8"
      style={{ aspectRatio: "52/39" }}
    >
      <div className="flex w-full flex-1 items-center justify-center gap-1">
        {audioBarHeights.map((height, i) => {
          const isTranscribed = i / audioBarHeights.length <= progress;
          return (
            <div
              key={i}
              className="flex-1 rounded-full transition-colors duration-300"
              style={{
                height: `${height * 100}%`,
                backgroundColor: isTranscribed ? "#ef4444" : "#f5f5f4",
                minWidth: "6px",
              }}
            />
          );
        })}
      </div>

      <div className="relative flex h-8 w-full items-center px-4">
        {words.map((word, i) => {
          const isVisible = progress >= word.position;
          return (
            <span
              key={i}
              className="absolute text-xs text-neutral-600 transition-opacity duration-300 sm:text-sm"
              style={{
                left: `${word.position * 100}%`,
                opacity: isVisible ? 1 : 0,
              }}
            >
              {word.text}
            </span>
          );
        })}
      </div>
    </div>
  );
}

function AnimatedMarkdownDemo({ isMobile = false }: { isMobile?: boolean }) {
  const [completedLines, setCompletedLines] = useState<React.ReactElement[]>(
    [],
  );
  const [currentLineIndex, setCurrentLineIndex] = useState(0);
  const [typingText, setTypingText] = useState("");
  const [isTransformed, setIsTransformed] = useState(false);
  const [showPlaceholder, setShowPlaceholder] = useState(false);

  const lines = [
    {
      text: "# Meeting Notes",
      type: "heading" as const,
      placeholder: "Enter header",
    },
    {
      text: "- Product roadmap review",
      type: "bullet" as const,
      placeholder: "Enter list item",
    },
    {
      text: "- Q4 marketing strategy",
      type: "bullet" as const,
      placeholder: "Enter list item",
    },
    {
      text: "- Budget allocation",
      type: "bullet" as const,
      placeholder: "Enter list item",
    },
    {
      text: "**Decision:** Launch campaign by end of month",
      type: "bold" as const,
      placeholder: "",
    },
  ];

  useEffect(() => {
    if (currentLineIndex >= lines.length) {
      const timeout = setTimeout(() => {
        setCompletedLines([]);
        setCurrentLineIndex(0);
        setTypingText("");
        setIsTransformed(false);
        setShowPlaceholder(false);
      }, 2000);
      return () => clearTimeout(timeout);
    }

    const currentLine = lines[currentLineIndex];
    let charIndex = 0;
    let timeout: NodeJS.Timeout;

    const typeCharacter = () => {
      if (charIndex < currentLine.text.length) {
        const newText = currentLine.text.slice(0, charIndex + 1);
        setTypingText(newText);
        charIndex++;

        // Check if we just typed the trigger sequence (e.g., "# " or "- ")
        const isMarkdownTrigger =
          (currentLine.type === "heading" && newText === "# ") ||
          (currentLine.type === "bullet" && newText === "- ");

        if (isMarkdownTrigger && !isTransformed) {
          // Show placeholder briefly, then transform
          setShowPlaceholder(true);

          timeout = setTimeout(() => {
            setIsTransformed(true);
            setShowPlaceholder(false);
            timeout = setTimeout(typeCharacter, 60);
          }, 300); // Show placeholder duration
        } else if (
          currentLine.type === "bold" &&
          newText.match(/\*\*[^*]+\*\*/)
        ) {
          setIsTransformed(true);
          timeout = setTimeout(typeCharacter, 60);
        } else {
          timeout = setTimeout(typeCharacter, 60);
        }
      } else {
        timeout = setTimeout(() => {
          const completedElement = renderCompletedLine(currentLine, isMobile);
          if (completedElement) {
            setCompletedLines((prev) => [...prev, completedElement]);
          }

          setTypingText("");
          setIsTransformed(false);
          setShowPlaceholder(false);
          setCurrentLineIndex((prev) => prev + 1);
        }, 800);
      }
    };

    typeCharacter();

    return () => clearTimeout(timeout);
  }, [currentLineIndex, isMobile]);

  const renderCompletedLine = (
    line: (typeof lines)[number],
    mobile: boolean,
  ) => {
    const key = `completed-${currentLineIndex}`;

    if (line.type === "heading") {
      const text = line.text.replace("# ", "");
      return (
        <h1
          key={key}
          className={cn([
            "font-bold text-stone-700",
            mobile ? "text-xl" : "text-2xl",
          ])}
        >
          {text}
        </h1>
      );
    }

    if (line.type === "bullet") {
      const text = line.text.replace("- ", "");
      return (
        <ul
          key={key}
          className={cn([
            "list-disc pl-5 text-neutral-700",
            mobile ? "text-sm" : "text-base",
          ])}
        >
          <li>{text}</li>
        </ul>
      );
    }

    if (line.type === "bold") {
      const parts = line.text.split(/(\*\*.*?\*\*)/g);
      return (
        <p
          key={key}
          className={cn(["text-neutral-700", mobile ? "text-sm" : "text-base"])}
        >
          {parts.map((part, i) => {
            if (part.startsWith("**") && part.endsWith("**")) {
              return (
                <span key={i} className="font-bold">
                  {part.slice(2, -2)}
                </span>
              );
            }
            return part;
          })}
        </p>
      );
    }

    return null;
  };

  const renderCurrentLine = () => {
    const currentLine = lines[currentLineIndex];

    if (!currentLine) {
      return null;
    }

    // Show placeholder state (after typing "# " or "- " but before transformation)
    if (showPlaceholder && !isTransformed) {
      // For headings, show with larger font size
      if (currentLine.type === "heading") {
        return (
          <h1 className={cn(["font-bold", isMobile ? "text-xl" : "text-2xl"])}>
            <span className="animate-pulse">|</span>
            <span className="text-neutral-400">{currentLine.placeholder}</span>
          </h1>
        );
      }

      // For bullets, show as list item
      if (currentLine.type === "bullet") {
        return (
          <ul
            className={cn([
              "list-disc pl-5",
              isMobile ? "text-sm" : "text-base",
            ])}
          >
            <li>
              <span className="animate-pulse">|</span>
              <span className="text-neutral-400">
                {currentLine.placeholder}
              </span>
            </li>
          </ul>
        );
      }

      // Default fallback (shouldn't reach here for current lines)
      return (
        <div
          className={cn([
            "text-neutral-700",
            isMobile ? "text-sm" : "text-base",
          ])}
        >
          <span className="animate-pulse">|</span>
          <span className="text-neutral-400">{currentLine.placeholder}</span>
        </div>
      );
    }

    // Transformed state for headings
    if (currentLine.type === "heading" && isTransformed) {
      const displayText = typingText.slice(2); // Remove "# "
      return (
        <h1
          className={cn([
            "font-bold text-stone-700",
            isMobile ? "text-xl" : "text-2xl",
          ])}
        >
          {displayText}
          <span className="animate-pulse">|</span>
        </h1>
      );
    }

    // Transformed state for bullets
    if (currentLine.type === "bullet" && isTransformed) {
      const displayText = typingText.slice(2); // Remove "- "
      return (
        <ul
          className={cn([
            "list-disc pl-5 text-neutral-700",
            isMobile ? "text-sm" : "text-base",
          ])}
        >
          <li>
            {displayText}
            <span className="animate-pulse">|</span>
          </li>
        </ul>
      );
    }

    // Transformed state for bold text
    if (currentLine.type === "bold" && isTransformed) {
      const parts = typingText.split(/(\*\*.*?\*\*)/g);
      return (
        <p
          className={cn([
            "text-neutral-700",
            isMobile ? "text-sm" : "text-base",
          ])}
        >
          {parts.map((part, i) => {
            if (part.startsWith("**") && part.endsWith("**")) {
              return (
                <span key={i} className="font-bold">
                  {part.slice(2, -2)}
                </span>
              );
            }
            return part;
          })}
          <span className="animate-pulse">|</span>
        </p>
      );
    }

    // Default typing state
    return (
      <div
        className={cn(["text-neutral-700", isMobile ? "text-sm" : "text-base"])}
      >
        {typingText}
        <span className="animate-pulse">|</span>
      </div>
    );
  };

  return (
    <div className={cn(["flex flex-col gap-3", isMobile && "gap-2"])}>
      {completedLines}
      {currentLineIndex < lines.length && renderCurrentLine()}
    </div>
  );
}

function TranscriptionSection() {
  return (
    <section id="transcription" className="border-y border-neutral-100">
      <div className="px-4 py-12 text-center lg:px-0">
        <h2 className="mb-4 font-serif text-3xl text-stone-700">
          Live meetings to recorded audio, Char transcribes it all
        </h2>
      </div>

      <div className="border-t border-neutral-100">
        <div className="hidden sm:grid sm:grid-cols-2">
          <div className="flex flex-col border-r border-neutral-100">
            <div className="flex flex-col gap-4 p-8">
              <div className="flex items-center gap-3">
                <Icon
                  icon="mdi:microphone"
                  className="text-3xl text-stone-700"
                />
                <h3 className="font-serif text-2xl text-stone-700">
                  Real-time transcription
                </h3>
              </div>
              <p className="text-base leading-relaxed text-neutral-600">
                Every word captured as it's spoken. See the transcript build in
                real-time with speaker identification and timestamps.
              </p>
            </div>
            <div className="flex flex-1 items-center justify-center overflow-hidden">
              <img
                src="/api/images/hyprnote/no-wifi.png"
                alt="On-device transcription"
                className="h-full w-full object-contain"
              />
            </div>
          </div>
          <div className="flex flex-col">
            <div className="flex flex-col gap-4 p-8">
              <div className="flex items-center gap-3">
                <Icon icon="mdi:upload" className="text-3xl text-stone-700" />
                <h3 className="font-serif text-2xl text-stone-700">
                  Upload files
                </h3>
              </div>
              <p className="text-base leading-relaxed text-neutral-600">
                Upload audio files (M4A, MP3, WAV) or existing transcripts (VTT,
                TXT) to get AI summaries and insights.
              </p>
            </div>
            <div className="flex flex-1 items-center justify-center overflow-hidden bg-neutral-100">
              <AudioTranscriptionDemo />
            </div>
          </div>
        </div>

        <div className="sm:hidden">
          <div className="border-b border-neutral-100">
            <div className="p-6">
              <div className="mb-3 flex items-center gap-3">
                <Icon
                  icon="mdi:microphone"
                  className="text-2xl text-stone-700"
                />
                <h3 className="font-serif text-xl text-stone-700">
                  Real-time transcription
                </h3>
              </div>
              <p className="mb-4 text-sm leading-relaxed text-neutral-600">
                Every word captured as it's spoken. See the transcript build in
                real-time with speaker identification and timestamps.
              </p>
            </div>
          </div>
          <div>
            <div className="p-6">
              <div className="mb-3 flex items-center gap-3">
                <Icon icon="mdi:upload" className="text-2xl text-stone-700" />
                <h3 className="font-serif text-xl text-stone-700">
                  Upload files
                </h3>
              </div>
              <p className="mb-4 text-sm leading-relaxed text-neutral-600">
                Upload audio files (M4A, MP3, WAV) or existing transcripts (VTT,
                TXT) to get AI summaries and insights.
              </p>
            </div>
            <div className="overflow-hidden bg-neutral-100">
              <AudioTranscriptionDemo />
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function SummariesSection() {
  const [typedText1, setTypedText1] = useState("");
  const [typedText2, setTypedText2] = useState("");
  const [enhancedLines, setEnhancedLines] = useState(0);

  const text1 = "metrisc w/ john";
  const text2 = "stakehlder mtg";

  useEffect(() => {
    const runAnimation = () => {
      setTypedText1("");
      setTypedText2("");
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
                            setTimeout(() => {
                              setEnhancedLines(7);
                              setTimeout(() => runAnimation(), 1000);
                            }, 800);
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
  }, []);

  return (
    <section id="summaries">
      <div className="px-4 py-12 text-center lg:px-0">
        <h2 className="mb-4 font-serif text-3xl text-stone-700">
          Your Notes+AI = Perfect Summary
        </h2>
      </div>
      <div className="border-t border-neutral-100">
        <div className="hidden sm:grid sm:grid-cols-2">
          <div className="flex flex-col overflow-clip border-r border-neutral-100">
            <div className="flex flex-col gap-4 p-8">
              <p className="font-serif text-lg leading-relaxed text-neutral-600">
                <span className="font-semibold">While you take notes,</span>{" "}
                Char listens and keeps track of everything that happens during
                the meeting.
              </p>
            </div>
            <div className="flex flex-1 items-end justify-center bg-stone-50/30 px-8 pb-0">
              <MockWindow showAudioIndicator={enhancedLines === 0}>
                <div className="h-[300px] overflow-hidden p-6">
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
            </div>
          </div>

          <div className="flex flex-col overflow-clip">
            <div className="flex flex-col gap-4 p-8">
              <p className="font-serif text-lg leading-relaxed text-neutral-600">
                <span className="font-semibold">
                  After the meeting is over,
                </span>{" "}
                Char combines your notes with transcripts to create a summary.
              </p>
            </div>
            <div className="flex flex-1 items-end justify-center bg-stone-50/30 px-8 pb-0">
              <MockWindow>
                <div className="flex h-[300px] flex-col gap-4 overflow-hidden p-6">
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
                        Sarah presented the new mobile UI update, which includes
                        a streamlined navigation bar and improved button
                        placements for better accessibility.
                      </li>
                      <li
                        className={cn([
                          "transition-opacity duration-500",
                          enhancedLines >= 3 ? "opacity-100" : "opacity-0",
                        ])}
                      >
                        Ben confirmed that API adjustments are needed to support
                        dynamic UI changes, particularly for fetching
                        personalized user data more efficiently.
                      </li>
                      <li
                        className={cn([
                          "transition-opacity duration-500",
                          enhancedLines >= 4 ? "opacity-100" : "opacity-0",
                        ])}
                      >
                        The UI update will be implemented in phases, starting
                        with core navigation improvements. Ben will ensure API
                        modifications are completed before development begins.
                      </li>
                    </ul>
                  </div>
                  <div className="flex flex-col gap-2">
                    <h4
                      className={cn([
                        "font-semibold text-stone-700 transition-opacity duration-500",
                        enhancedLines >= 5 ? "opacity-100" : "opacity-0",
                      ])}
                    >
                      New Dashboard – Urgent Priority
                    </h4>
                    <ul className="flex list-disc flex-col gap-2 pl-5 text-sm text-neutral-700">
                      <li
                        className={cn([
                          "transition-opacity duration-500",
                          enhancedLines >= 6 ? "opacity-100" : "opacity-0",
                        ])}
                      >
                        Alice emphasized that the new analytics dashboard must
                        be prioritized due to increasing stakeholder demand.
                      </li>
                      <li
                        className={cn([
                          "transition-opacity duration-500",
                          enhancedLines >= 7 ? "opacity-100" : "opacity-0",
                        ])}
                      >
                        The new dashboard will feature real-time user engagement
                        metrics and a customizable reporting system.
                      </li>
                    </ul>
                  </div>
                </div>
              </MockWindow>
            </div>
          </div>
        </div>

        <div className="sm:hidden">
          <div className="border-b border-neutral-100">
            <div className="p-6 pb-2">
              <p className="mb-4 font-serif text-base leading-relaxed text-neutral-600">
                <span className="font-semibold">While you take notes,</span>{" "}
                Char listens and keeps track of everything that happens during
                the meeting.
              </p>
            </div>
            <div className="overflow-clip bg-stone-50/30 px-6 pb-0">
              <MockWindow variant="mobile">
                <div className="h-[200px] overflow-hidden p-6">
                  <div className="text-neutral-700">ui update - mobile</div>
                  <div className="text-neutral-700">api</div>
                  <div className="mt-3 text-neutral-700">new dash - urgent</div>
                  <div className="text-neutral-700">a/b test next wk</div>
                  <div className="mt-3 text-neutral-700">
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
            </div>
          </div>

          <div>
            <div className="p-6 pb-2">
              <p className="mb-4 font-serif text-base leading-relaxed text-neutral-600">
                <span className="font-semibold">
                  After the meeting is over,
                </span>{" "}
                Char combines your notes with transcripts to create a summary.
              </p>
            </div>
            <div className="overflow-clip bg-stone-50/30 px-6 pb-0">
              <MockWindow variant="mobile">
                <div className="flex h-[200px] flex-col gap-4 overflow-hidden p-6">
                  <div className="flex flex-col gap-2">
                    <h4 className="text-lg font-semibold text-stone-700">
                      Mobile UI Update and API Adjustments
                    </h4>
                    <ul className="flex list-disc flex-col gap-2 pl-4 text-neutral-700">
                      <li
                        className={cn([
                          "transition-opacity duration-500",
                          enhancedLines >= 1 ? "opacity-100" : "opacity-0",
                        ])}
                      >
                        Sarah presented the new mobile UI update, which includes
                        a streamlined navigation bar and improved button
                        placements for better accessibility.
                      </li>
                      <li
                        className={cn([
                          "transition-opacity duration-500",
                          enhancedLines >= 2 ? "opacity-100" : "opacity-0",
                        ])}
                      >
                        Ben confirmed that API adjustments are needed to support
                        dynamic UI changes, particularly for fetching
                        personalized user data more efficiently.
                      </li>
                      <li
                        className={cn([
                          "transition-opacity duration-500",
                          enhancedLines >= 3 ? "opacity-100" : "opacity-0",
                        ])}
                      >
                        The UI update will be implemented in phases, starting
                        with core navigation improvements. Ben will ensure API
                        modifications are completed before development begins.
                      </li>
                    </ul>
                  </div>
                  <div className="flex flex-col gap-2">
                    <h4 className="text-lg font-semibold text-stone-700">
                      New Dashboard – Urgent Priority
                    </h4>
                    <ul className="flex list-disc flex-col gap-2 pl-4 text-neutral-700">
                      <li
                        className={cn([
                          "transition-opacity duration-500",
                          enhancedLines >= 4 ? "opacity-100" : "opacity-0",
                        ])}
                      >
                        Alice emphasized that the new analytics dashboard must
                        be prioritized due to increasing stakeholder demand.
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
                    </ul>
                  </div>
                </div>
              </MockWindow>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function SearchSection() {
  const searchQueries = [
    "Q3 marketing strategy discussion",
    "client feedback on product demo",
    "budget planning for next quarter",
    "project timeline with Sarah",
    "brainstorming session notes",
  ];

  return (
    <section
      id="search"
      className="bg-stone-50/30 bg-cover bg-center"
      style={{
        backgroundImage: "url(/api/images/texture/bg-stars.jpg)",
      }}
    >
      <div className="px-6 py-20">
        <div className="flex flex-col gap-12 text-center">
          <div>
            <h2 className="mb-4 font-serif text-3xl text-stone-50">
              Find anything instantly
            </h2>
            <p className="text-base text-neutral-100">
              Search across all your notes by participant names, topics,
              keywords, or time—and jump straight to what matters
            </p>
          </div>

          <div className="relative mx-auto flex max-w-2xl flex-col gap-3">
            <div className="flex items-center gap-3 rounded-full border border-stone-300 bg-white px-4 py-3 shadow-[0_4px_6px_-1px_rgba(255,255,255,0.3),0_2px_4px_-2px_rgba(255,255,255,0.3)]">
              <SearchIcon className="size-5 shrink-0 text-stone-400" />
              <div className="min-w-0 flex-1 overflow-hidden text-left">
                <Typewriter
                  text={searchQueries}
                  speed={100}
                  deleteSpeed={30}
                  waitTime={2000}
                  className="block truncate text-base font-light text-stone-700 sm:text-lg"
                  cursorClassName="ml-1"
                />
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

const CollaboratorsCell = memo(() => {
  const [showDavid, setShowDavid] = useState(false);
  const [davidScope, setDavidScope] = useState("Can view");
  const [showPopover, setShowPopover] = useState(false);

  const baseCollaborators = [
    {
      name: "Alex Johnson",
      avatar: "/api/images/mock/alex-johnson.png",
      scope: "Can view",
    },
    {
      name: "Jessica Lee",
      avatar: "/api/images/mock/jessica-lee.png",
      scope: "Can edit",
    },
    {
      name: "Sarah Chen",
      avatar: "/api/images/mock/sarah-chen.png",
      scope: "Can edit",
    },
    {
      name: "Michael Park",
      avatar: "/api/images/mock/michael-park.png",
      scope: "Can view",
    },
    {
      name: "Emily Rodriguez",
      avatar: "/api/images/mock/emily-rodriguez.png",
      scope: "Can edit",
    },
  ];

  const davidKim = {
    name: "David Kim",
    avatar: "/api/images/mock/david-kim.png",
    scope: davidScope,
  };

  const collaborators = showDavid
    ? [...baseCollaborators, davidKim]
    : baseCollaborators;

  useEffect(() => {
    const runAnimation = () => {
      setShowDavid(false);
      setShowPopover(false);
      setDavidScope("Can view");

      const timer1 = setTimeout(() => setShowDavid(true), 2000);
      const timer2 = setTimeout(() => setShowPopover(true), 4000);
      const timer3 = setTimeout(() => {
        setDavidScope("Can comment");
        setShowPopover(false);
      }, 5000);
      const timer4 = setTimeout(() => runAnimation(), 8000);

      return () => {
        clearTimeout(timer1);
        clearTimeout(timer2);
        clearTimeout(timer3);
        clearTimeout(timer4);
      };
    };

    const cleanup = runAnimation();
    return cleanup;
  }, []);

  return (
    <>
      <div className="h-[300px] overflow-hidden p-4 sm:aspect-4/3 sm:h-auto">
        <div className="flex h-full items-end">
          <div className="flex w-full flex-col gap-2">
            <AnimatePresence>
              {collaborators.map((person) => (
                <motion.div
                  key={person.name}
                  initial={
                    person.name === "David Kim"
                      ? { opacity: 0, x: -100 }
                      : false
                  }
                  animate={{ opacity: 1, x: 0 }}
                  transition={{
                    duration: 0.5,
                    ease: "easeOut",
                  }}
                  className="flex items-center gap-3 rounded-lg border border-stone-200/50 bg-linear-to-br from-stone-50/80 to-white/80 p-3 backdrop-blur-xs"
                >
                  <Image
                    src={person.avatar}
                    alt={person.name}
                    width={32}
                    height={32}
                    className="shrink-0 rounded-full"
                    objectFit="cover"
                  />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium text-stone-700">
                      {person.name}
                    </div>
                  </div>
                  <motion.div
                    key={`${person.name}-${person.scope}`}
                    initial={
                      person.name === "David Kim" &&
                      davidScope === "Can comment"
                        ? { scale: 1.1 }
                        : false
                    }
                    animate={{ scale: 1 }}
                    transition={{ duration: 0.3 }}
                    className="relative w-32 shrink-0"
                  >
                    <div className="flex items-center gap-1 rounded border border-stone-200 bg-white px-2 py-1 text-xs text-neutral-700">
                      <span className="flex-1 truncate">{person.scope}</span>
                      <ChevronDownIcon className="h-4 w-4 shrink-0 text-neutral-400" />
                    </div>
                    <AnimatePresence>
                      {person.name === "David Kim" && showPopover && (
                        <motion.div
                          initial={{
                            opacity: 0,
                            y: 10,
                          }}
                          animate={{
                            opacity: 1,
                            y: 0,
                          }}
                          exit={{
                            opacity: 0,
                            y: 10,
                          }}
                          transition={{
                            duration: 0.2,
                          }}
                          className="absolute bottom-full left-0 z-20 mb-1 w-32 overflow-hidden rounded border border-stone-200 bg-white shadow-lg"
                        >
                          <div
                            className={cn([
                              "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                              davidScope === "Can view" && "bg-stone-50",
                            ])}
                          >
                            <CheckIcon
                              className={cn([
                                "h-4 w-4",
                                davidScope === "Can view"
                                  ? "text-green-600"
                                  : "text-transparent",
                              ])}
                            />
                            <span>Can view</span>
                          </div>
                          <div
                            className={cn([
                              "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                              davidScope === "Can comment" && "bg-stone-50",
                            ])}
                          >
                            <CheckIcon
                              className={cn([
                                "h-4 w-4",
                                davidScope === "Can comment"
                                  ? "text-green-600"
                                  : "text-transparent",
                              ])}
                            />
                            <span>Can comment</span>
                          </div>
                          <div className="flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50">
                            <CheckIcon className="h-4 w-4 text-transparent" />
                            <span>Can edit</span>
                          </div>
                        </motion.div>
                      )}
                    </AnimatePresence>
                  </motion.div>
                </motion.div>
              ))}
            </AnimatePresence>
          </div>
        </div>
      </div>
    </>
  );
});

CollaboratorsCell.displayName = "CollaboratorsCell";

const ShareLinksCell = memo(() => {
  const [linkClicked, setLinkClicked] = useState(false);
  const [showCopied, setShowCopied] = useState(false);
  const [linkPermission, setLinkPermission] = useState("View only");
  const [showLinkPopover, setShowLinkPopover] = useState(false);
  const [slackClicked, setSlackClicked] = useState(false);
  const [showSlackPopover, setShowSlackPopover] = useState(false);
  const [selectedChannel, setSelectedChannel] = useState("");
  const [sendClicked, setSendClicked] = useState(false);
  const [showSent, setShowSent] = useState(false);
  const [teamsClicked, setTeamsClicked] = useState(false);
  const [showTeamsPopover, setShowTeamsPopover] = useState(false);
  const [selectedTeam, setSelectedTeam] = useState("");
  const [teamsSendClicked, setTeamsSendClicked] = useState(false);
  const [teamsShowSent, setTeamsShowSent] = useState(false);
  const [salesforceClicked, setSalesforceClicked] = useState(false);
  const [showSalesforcePopover, setShowSalesforcePopover] = useState(false);
  const [selectedLead, setSelectedLead] = useState("");
  const [salesforceSendClicked, setSalesforceSendClicked] = useState(false);
  const [salesforceShowSent, setSalesforceShowSent] = useState(false);

  useEffect(() => {
    const runAnimation = () => {
      setLinkClicked(false);
      setShowCopied(false);
      setLinkPermission("View only");
      setShowLinkPopover(false);
      setSlackClicked(false);
      setShowSlackPopover(false);
      setSelectedChannel("");
      setSendClicked(false);
      setShowSent(false);
      setTeamsClicked(false);
      setShowTeamsPopover(false);
      setSelectedTeam("");
      setTeamsSendClicked(false);
      setTeamsShowSent(false);
      setSalesforceClicked(false);
      setShowSalesforcePopover(false);
      setSelectedLead("");
      setSalesforceSendClicked(false);
      setSalesforceShowSent(false);

      const timer1 = setTimeout(() => setShowLinkPopover(true), 2000);
      const timer2 = setTimeout(() => setLinkPermission("Editable"), 2500);
      const timer3 = setTimeout(() => setShowLinkPopover(false), 2800);
      const timer4 = setTimeout(() => setLinkClicked(true), 3300);
      const timer5 = setTimeout(() => setShowCopied(true), 3600);
      const timer6 = setTimeout(() => setSlackClicked(true), 4500);
      const timer7 = setTimeout(() => setShowSlackPopover(true), 4800);
      const timer8 = setTimeout(
        () => setSelectedChannel("#team-meeting"),
        5500,
      );
      const timer9 = setTimeout(() => setShowSlackPopover(false), 5800);
      const timer10 = setTimeout(() => setSendClicked(true), 6100);
      const timer11 = setTimeout(() => setShowSent(true), 6400);
      const timer12 = setTimeout(() => setTeamsClicked(true), 7000);
      const timer13 = setTimeout(() => setShowTeamsPopover(true), 7300);
      const timer14 = setTimeout(() => setSelectedTeam("Design Team"), 8000);
      const timer15 = setTimeout(() => setShowTeamsPopover(false), 8300);
      const timer16 = setTimeout(() => setTeamsSendClicked(true), 8600);
      const timer17 = setTimeout(() => setTeamsShowSent(true), 8900);
      const timer18 = setTimeout(() => setSalesforceClicked(true), 9500);
      const timer19 = setTimeout(() => setShowSalesforcePopover(true), 9800);
      const timer20 = setTimeout(() => setSelectedLead("John Smith"), 10500);
      const timer21 = setTimeout(() => setShowSalesforcePopover(false), 10800);
      const timer22 = setTimeout(() => setSalesforceSendClicked(true), 11100);
      const timer23 = setTimeout(() => setSalesforceShowSent(true), 11400);
      const timer24 = setTimeout(() => runAnimation(), 13000);

      return () => {
        clearTimeout(timer1);
        clearTimeout(timer2);
        clearTimeout(timer3);
        clearTimeout(timer4);
        clearTimeout(timer5);
        clearTimeout(timer6);
        clearTimeout(timer7);
        clearTimeout(timer8);
        clearTimeout(timer9);
        clearTimeout(timer10);
        clearTimeout(timer11);
        clearTimeout(timer12);
        clearTimeout(timer13);
        clearTimeout(timer14);
        clearTimeout(timer15);
        clearTimeout(timer16);
        clearTimeout(timer17);
        clearTimeout(timer18);
        clearTimeout(timer19);
        clearTimeout(timer20);
        clearTimeout(timer21);
        clearTimeout(timer22);
        clearTimeout(timer23);
        clearTimeout(timer24);
      };
    };

    const cleanup = runAnimation();
    return cleanup;
  }, []);

  return (
    <div className="flex h-[300px] items-center justify-center overflow-hidden p-4 sm:aspect-4/3 sm:h-auto">
      <div className="flex w-full flex-col gap-2">
        <motion.div
          animate={linkClicked ? { scale: [1, 0.95, 1] } : {}}
          transition={{ duration: 0.3 }}
          className={cn([
            "relative flex cursor-pointer items-center justify-between gap-3 overflow-visible rounded-lg border border-stone-200/50 bg-linear-to-br from-purple-50/80 to-white/80 p-3 backdrop-blur-xs",
            showLinkPopover && "z-10",
          ])}
        >
          <Icon icon="hugeicons:note" className="w-8 text-stone-700" />
          <div className="relative flex flex-1 items-center justify-between gap-2">
            <motion.div
              key={linkPermission}
              initial={
                linkPermission !== "View only" ? { scale: 1.1 } : { scale: 1 }
              }
              animate={{ scale: 1 }}
              transition={{ duration: 0.3 }}
              className="relative flex w-32 items-center gap-1 rounded border border-stone-200 bg-white px-2 py-1 text-xs text-neutral-700"
            >
              <span className="flex-1 truncate">{linkPermission}</span>
              <ChevronDownIcon className="h-4 w-4 shrink-0 text-neutral-400" />
            </motion.div>
            <AnimatePresence>
              {showLinkPopover && (
                <motion.div
                  initial={{ opacity: 0, y: -10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -10 }}
                  transition={{ duration: 0.2 }}
                  className="absolute top-full left-0 z-20 mt-1 w-32 overflow-hidden rounded border border-stone-200 bg-white shadow-lg"
                >
                  <div
                    className={cn([
                      "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                      linkPermission === "Restricted" && "bg-stone-50",
                    ])}
                  >
                    <CheckIcon
                      className={cn([
                        "h-4 w-4",
                        linkPermission === "Restricted"
                          ? "text-green-600"
                          : "text-transparent",
                      ])}
                    />
                    <span>Restricted</span>
                  </div>
                  <div
                    className={cn([
                      "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                      linkPermission === "View only" && "bg-stone-50",
                    ])}
                  >
                    <CheckIcon
                      className={cn([
                        "h-4 w-4",
                        linkPermission === "View only"
                          ? "text-green-600"
                          : "text-transparent",
                      ])}
                    />
                    <span>View only</span>
                  </div>
                  <div
                    className={cn([
                      "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                      linkPermission === "Editable" && "bg-stone-50",
                    ])}
                  >
                    <CheckIcon
                      className={cn([
                        "h-4 w-4",
                        linkPermission === "Editable"
                          ? "text-green-600"
                          : "text-transparent",
                      ])}
                    />
                    <span>Editable</span>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
            <motion.button
              key={showCopied ? "copied" : "copy"}
              animate={linkClicked ? { scale: [1, 0.95, 1] } : {}}
              transition={{ duration: 0.3 }}
              className={cn([
                "flex w-24 items-center justify-center gap-1.5 rounded-full px-3 py-2 text-xs font-medium transition-all",
                showCopied
                  ? "bg-linear-to-t from-stone-600 to-stone-500 text-white"
                  : "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 hover:scale-105 active:scale-95",
              ])}
            >
              {showCopied && <CheckIcon className="h-4 w-4 shrink-0" />}
              <span>{showCopied ? "Copied" : "Copy"}</span>
            </motion.button>
          </div>
        </motion.div>
        <motion.div
          animate={slackClicked ? { scale: [1, 0.95, 1] } : {}}
          transition={{ duration: 0.3 }}
          className={cn([
            "relative flex cursor-pointer items-center gap-3 overflow-visible rounded-lg border border-stone-200/50 bg-linear-to-br from-green-50/80 to-white/80 p-3 backdrop-blur-xs",
            showSlackPopover && "z-10",
          ])}
        >
          <Icon icon="logos:slack-icon" className="w-8" />
          <div className="relative flex flex-1 items-center justify-between gap-2">
            <motion.div
              key={selectedChannel || "default"}
              initial={selectedChannel ? { scale: 1.1 } : { scale: 1 }}
              animate={{ scale: 1 }}
              transition={{ duration: 0.3 }}
              className="relative flex w-32 items-center gap-1 rounded border border-stone-200 bg-white px-2 py-1 text-xs text-neutral-700"
            >
              <span className="flex-1 truncate">
                {selectedChannel || "Select channel"}
              </span>
              <ChevronDownIcon className="h-4 w-4 shrink-0 text-neutral-400" />
            </motion.div>
            <AnimatePresence>
              {showSlackPopover && (
                <motion.div
                  initial={{ opacity: 0, y: -10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -10 }}
                  transition={{ duration: 0.2 }}
                  className="absolute top-full left-0 z-20 mt-1 w-40 overflow-hidden rounded border border-stone-200 bg-white shadow-lg"
                >
                  <div
                    className={cn([
                      "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                      selectedChannel === "#team-meeting" && "bg-stone-50",
                    ])}
                  >
                    <CheckIcon
                      className={cn([
                        "h-4 w-4",
                        selectedChannel === "#team-meeting"
                          ? "text-green-600"
                          : "text-transparent",
                      ])}
                    />
                    <span>#team-meeting</span>
                  </div>
                  <div className="flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50">
                    <CheckIcon className="h-4 w-4 text-transparent" />
                    <span>#marketing</span>
                  </div>
                  <div className="flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50">
                    <CheckIcon className="h-4 w-4 text-transparent" />
                    <span>#general</span>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
            <motion.button
              key={showSent ? "sent" : "send"}
              animate={sendClicked ? { scale: [1, 0.95, 1] } : {}}
              transition={{ duration: 0.3 }}
              className={cn([
                "flex w-24 items-center justify-center rounded-full px-3 py-2 text-xs font-medium transition-all",
                showSent
                  ? "bg-linear-to-t from-stone-600 to-stone-500 text-white"
                  : "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 hover:scale-105 active:scale-95",
              ])}
            >
              {showSent ? (
                <span className="flex items-center justify-center gap-1.5">
                  <CheckIcon className="h-4 w-4 shrink-0" />
                  Sent
                </span>
              ) : (
                "Send"
              )}
            </motion.button>
          </div>
        </motion.div>
        <motion.div
          animate={teamsClicked ? { scale: [1, 0.95, 1] } : {}}
          transition={{ duration: 0.3 }}
          className="relative flex cursor-pointer items-center gap-3 overflow-visible rounded-lg border border-stone-200/50 bg-linear-to-br from-indigo-50/80 to-white/80 p-3 backdrop-blur-xs"
        >
          <Icon icon="logos:microsoft-teams" className="w-8" />
          <div className="relative flex flex-1 items-center justify-between gap-2">
            <motion.div
              key={selectedTeam || "default"}
              initial={selectedTeam ? { scale: 1.1 } : { scale: 1 }}
              animate={{ scale: 1 }}
              transition={{ duration: 0.3 }}
              className="relative flex w-32 items-center gap-1 rounded border border-stone-200 bg-white px-2 py-1 text-xs text-neutral-700"
            >
              <span className="flex-1 truncate">
                {selectedTeam || "Select team"}
              </span>
              <ChevronDownIcon className="h-4 w-4 shrink-0 text-neutral-400" />
            </motion.div>
            <AnimatePresence>
              {showTeamsPopover && (
                <motion.div
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: 10 }}
                  transition={{ duration: 0.2 }}
                  className="absolute bottom-full left-0 z-20 mb-1 w-32 overflow-hidden rounded border border-stone-200 bg-white shadow-lg"
                >
                  <div
                    className={cn([
                      "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                      selectedTeam === "Design Team" && "bg-stone-50",
                    ])}
                  >
                    <CheckIcon
                      className={cn([
                        "h-4 w-4",
                        selectedTeam === "Design Team"
                          ? "text-green-600"
                          : "text-transparent",
                      ])}
                    />
                    <span>Design Team</span>
                  </div>
                  <div className="flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50">
                    <CheckIcon className="h-4 w-4 text-transparent" />
                    <span>Engineering</span>
                  </div>
                  <div className="flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50">
                    <CheckIcon className="h-4 w-4 text-transparent" />
                    <span>Marketing</span>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
            <motion.button
              key={teamsShowSent ? "sent" : "send"}
              animate={teamsSendClicked ? { scale: [1, 0.95, 1] } : {}}
              transition={{ duration: 0.3 }}
              className={cn([
                "flex w-24 items-center justify-center rounded-full px-3 py-2 text-xs font-medium transition-all",
                teamsShowSent
                  ? "bg-linear-to-t from-stone-600 to-stone-500 text-white"
                  : "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 hover:scale-105 active:scale-95",
              ])}
            >
              {teamsShowSent ? (
                <span className="flex items-center justify-center gap-1.5">
                  <CheckIcon className="h-4 w-4 shrink-0" />
                  Sent
                </span>
              ) : (
                "Send"
              )}
            </motion.button>
          </div>
        </motion.div>
        <motion.div
          animate={salesforceClicked ? { scale: [1, 0.95, 1] } : {}}
          transition={{ duration: 0.3 }}
          className="relative flex cursor-pointer items-center gap-3 overflow-visible rounded-lg border border-stone-200/50 bg-linear-to-br from-cyan-50/80 to-white/80 p-3 backdrop-blur-xs"
        >
          <Icon icon="logos:salesforce" className="w-8" />
          <div className="relative flex flex-1 items-center justify-between gap-2">
            <motion.div
              key={selectedLead || "default"}
              initial={selectedLead ? { scale: 1.1 } : { scale: 1 }}
              animate={{ scale: 1 }}
              transition={{ duration: 0.3 }}
              className="relative flex w-32 items-center gap-1 rounded border border-stone-200 bg-white px-2 py-1 text-xs text-neutral-700"
            >
              <span className="flex-1 truncate">
                {selectedLead || "Select lead"}
              </span>
              <ChevronDownIcon className="h-4 w-4 shrink-0 text-neutral-400" />
            </motion.div>
            <AnimatePresence>
              {showSalesforcePopover && (
                <motion.div
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: 10 }}
                  transition={{ duration: 0.2 }}
                  className="absolute bottom-full left-0 z-20 mb-1 w-32 overflow-hidden rounded border border-stone-200 bg-white shadow-lg"
                >
                  <div
                    className={cn([
                      "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                      selectedLead === "John Smith" && "bg-stone-50",
                    ])}
                  >
                    <CheckIcon
                      className={cn([
                        "h-4 w-4",
                        selectedLead === "John Smith"
                          ? "text-green-600"
                          : "text-transparent",
                      ])}
                    />
                    <span>John Smith</span>
                  </div>
                  <div className="flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50">
                    <CheckIcon className="h-4 w-4 text-transparent" />
                    <span>Sarah Williams</span>
                  </div>
                  <div className="flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50">
                    <CheckIcon className="h-4 w-4 text-transparent" />
                    <span>Mike Anderson</span>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
            <motion.button
              key={salesforceShowSent ? "synced" : "sync"}
              animate={salesforceSendClicked ? { scale: [1, 0.95, 1] } : {}}
              transition={{ duration: 0.3 }}
              className={cn([
                "flex w-24 items-center justify-center rounded-full px-3 py-2 text-xs font-medium transition-all",
                salesforceShowSent
                  ? "bg-linear-to-t from-stone-600 to-stone-500 text-white"
                  : "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 hover:scale-105 active:scale-95",
              ])}
            >
              {salesforceShowSent ? (
                <span className="flex items-center justify-center gap-1.5">
                  <CheckIcon className="h-4 w-4 shrink-0" />
                  Synced
                </span>
              ) : (
                "Sync"
              )}
            </motion.button>
          </div>
        </motion.div>
      </div>
    </div>
  );
});

ShareLinksCell.displayName = "ShareLinksCell";

const TrackProtectCell = memo(() => {
  const [countdown, setCountdown] = useState(3);
  const [showNote, setShowNote] = useState(true);
  const [showShatter, setShowShatter] = useState(false);

  useEffect(() => {
    const runAnimation = () => {
      setCountdown(3);
      setShowNote(true);
      setShowShatter(false);

      const countdownInterval = setInterval(() => {
        setCountdown((prev) => {
          if (prev <= 1) {
            clearInterval(countdownInterval);
            return 0;
          }
          return prev - 1;
        });
      }, 1000);

      const shatterTimer = setTimeout(() => {
        setShowShatter(true);
        setShowNote(false);
        setTimeout(() => {
          setShowShatter(false);
          setTimeout(() => runAnimation(), 500);
        }, 800);
      }, 3000);

      return () => {
        clearInterval(countdownInterval);
        clearTimeout(shatterTimer);
      };
    };

    const cleanup = runAnimation();
    return cleanup;
  }, []);

  return (
    <div className="relative flex h-[300px] flex-col overflow-hidden bg-linear-to-br from-stone-50/30 to-stone-100/50 sm:aspect-4/3 sm:h-auto">
      <AnimatePresence>
        {countdown > 0 && showNote && (
          <motion.div
            initial={{ opacity: 0, scale: 0.8 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.8 }}
            key={countdown}
            className="absolute top-2 right-2 z-10 flex h-10 w-10 items-center justify-center rounded-full border-2 border-stone-400 bg-stone-600 text-sm font-semibold text-white shadow-md"
            style={{
              background: `conic-linear(#57534e 0deg ${(4 - countdown) * 120}deg, #78716c ${(4 - countdown) * 120}deg 360deg)`,
            }}
          >
            <div className="absolute inset-1 flex items-center justify-center rounded-full bg-stone-600">
              {countdown}
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <div className="relative flex-1">
        <AnimatePresence>
          {showNote && !showShatter && (
            <motion.div
              initial={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.1 }}
              className="relative h-full overflow-hidden bg-white p-4"
            >
              <div className="pointer-events-none absolute inset-0 overflow-hidden">
                {Array.from({ length: 8 }).map((_, i) => (
                  <div
                    key={i}
                    className="absolute text-xs font-medium whitespace-nowrap text-stone-300/30"
                    style={{
                      top: `${(i * 15) % 100}%`,
                      left: `${(i * 25) % 100}%`,
                      transform: "rotate(-45deg)",
                    }}
                  >
                    user@example.com
                  </div>
                ))}
              </div>

              <div className="relative flex flex-col gap-3">
                <div className="text-sm font-semibold text-stone-700">
                  Mobile UI Update
                </div>
                <div className="flex flex-col gap-2">
                  <div className="h-3 w-full rounded bg-stone-100" />
                  <div className="h-3 w-full rounded bg-stone-100" />
                  <div className="h-3 w-5/6 rounded bg-stone-100" />
                </div>
                <div className="mt-4 text-sm font-semibold text-stone-700">
                  Dashboard Priority
                </div>
                <div className="flex flex-col gap-2">
                  <div className="h-3 w-full rounded bg-stone-100" />
                  <div className="h-3 w-full rounded bg-stone-100" />
                  <div className="h-3 w-4/5 rounded bg-stone-100" />
                </div>
                <div className="mt-4 text-sm font-semibold text-stone-700">
                  Next Steps
                </div>
                <div className="flex flex-col gap-2">
                  <div className="h-3 w-full rounded bg-stone-100" />
                  <div className="h-3 w-5/6 rounded bg-stone-100" />
                </div>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        <AnimatePresence>
          {showShatter && (
            <div className="absolute inset-0 overflow-hidden bg-white">
              {Array.from({ length: 144 }).map((_, i) => {
                const row = Math.floor(i / 12);
                const col = i % 12;
                const x = col * 8.33;
                const y = row * 8.33;
                const randomX = (Math.random() - 0.5) * 300;
                const randomY = Math.random() * 400 + 200;
                const randomRotate = (Math.random() - 0.5) * 180;

                return (
                  <motion.div
                    key={i}
                    initial={{
                      position: "absolute",
                      left: `${x}%`,
                      top: `${y}%`,
                      width: "8.33%",
                      height: "8.33%",
                      backgroundColor: "#fff",
                      border: "1px solid #e7e5e4",
                    }}
                    animate={{
                      x: randomX,
                      y: randomY,
                      rotate: randomRotate,
                      opacity: 0,
                    }}
                    transition={{
                      duration: 0.8,
                      ease: "easeIn",
                    }}
                  />
                );
              })}
            </div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
});

TrackProtectCell.displayName = "TrackProtectCell";

function SharingSection() {
  return (
    <section id="sharing">
      <div className="px-4 py-12 text-center lg:px-0">
        <div className="mb-4 inline-block rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 px-4 py-1.5 text-xs font-medium text-neutral-900 shadow-xs">
          Coming Soon
        </div>
        <h2 className="mb-4 font-serif text-3xl text-stone-700">Share notes</h2>
        <p className="text-base text-neutral-600">
          Collaborate seamlessly by sharing meeting notes, transcripts, and
          summaries with your team.
        </p>
      </div>
      <div className="border-t border-neutral-100">
        <div className="hidden min-[1000px]:grid min-[1000px]:grid-cols-3">
          <div className="flex flex-col border-r border-neutral-100 bg-linear-to-b from-white to-stone-50/30">
            <div className="flex flex-1 flex-col gap-4 border-b border-neutral-100 p-4">
              <div className="flex items-center gap-3">
                <Icon
                  icon="mdi:account-group"
                  className="text-3xl text-stone-700"
                />
                <h3 className="font-serif text-2xl text-stone-700">
                  Control who can access
                </h3>
              </div>
              <p className="text-base leading-relaxed text-neutral-600">
                Invite selected people or teams to collaborate on notes with
                granular access controls.
              </p>
            </div>
            <CollaboratorsCell />
          </div>
          <div className="flex flex-col border-r border-neutral-100 bg-linear-to-b from-white to-stone-50/30">
            <div className="flex flex-1 flex-col gap-4 border-b border-neutral-100 p-4">
              <div className="flex items-center gap-3">
                <Icon
                  icon="mdi:link-variant"
                  className="text-3xl text-stone-700"
                />
                <h3 className="font-serif text-2xl text-stone-700">
                  Share instantly
                </h3>
              </div>
              <p className="text-base leading-relaxed text-neutral-600">
                Send links or publish notes directly to Slack, Teams, or
                generate public shareable links.
              </p>
            </div>
            <ShareLinksCell />
          </div>
          <div className="flex flex-col bg-linear-to-b from-white to-stone-50/30">
            <div className="flex flex-1 flex-col gap-4 border-b border-neutral-100 p-4">
              <div className="flex items-center gap-3">
                <Icon
                  icon="mdi:shield-lock"
                  className="text-3xl text-stone-700"
                />
                <h3 className="font-serif text-2xl text-stone-700">
                  Track and protect
                </h3>
              </div>
              <p className="text-base leading-relaxed text-neutral-600">
                DocSend-like features including view tracking, expiration dates,
                copy protection, and watermarks.
              </p>
            </div>
            <TrackProtectCell />
          </div>
        </div>

        <div className="hidden overflow-x-auto min-[1000px]:hidden! sm:block">
          <div className="flex min-w-max">
            <div className="flex w-[400px] flex-col border-r border-neutral-100 bg-linear-to-b from-white to-stone-50/30">
              <div className="flex flex-1 flex-col gap-4 border-b border-neutral-100 p-4">
                <div className="flex items-center gap-3">
                  <Icon
                    icon="mdi:account-group"
                    className="text-3xl text-stone-700"
                  />
                  <h3 className="font-serif text-2xl text-stone-700">
                    Control who can access
                  </h3>
                </div>
                <p className="text-base leading-relaxed text-neutral-600">
                  Invite selected people or teams to collaborate on notes with
                  granular access controls.
                </p>
              </div>
              <CollaboratorsCell />
            </div>
            <div className="flex w-[400px] flex-col border-r border-neutral-100 bg-linear-to-b from-white to-stone-50/30">
              <div className="flex flex-1 flex-col gap-4 border-b border-neutral-100 p-4">
                <div className="flex items-center gap-3">
                  <Icon
                    icon="mdi:link-variant"
                    className="text-3xl text-stone-700"
                  />
                  <h3 className="font-serif text-2xl text-stone-700">
                    Share instantly
                  </h3>
                </div>
                <p className="text-base leading-relaxed text-neutral-600">
                  Send links or publish notes directly to Slack, Teams, or
                  generate public shareable links.
                </p>
              </div>
              <ShareLinksCell />
            </div>
            <div className="flex w-[400px] flex-col bg-linear-to-b from-white to-stone-50/30">
              <div className="flex flex-1 flex-col gap-4 border-b border-neutral-100 p-4">
                <div className="flex items-center gap-3">
                  <Icon
                    icon="mdi:shield-lock"
                    className="text-3xl text-stone-700"
                  />
                  <h3 className="font-serif text-2xl text-stone-700">
                    Track and protect
                  </h3>
                </div>
                <p className="text-base leading-relaxed text-neutral-600">
                  DocSend-like features including view tracking, expiration
                  dates, copy protection, and watermarks.
                </p>
              </div>
              <TrackProtectCell />
            </div>
          </div>
        </div>

        <div className="sm:hidden">
          <div className="border-b border-neutral-100 bg-linear-to-b from-white to-stone-50/30">
            <div className="border-b border-neutral-100 p-4">
              <div className="mb-3 flex items-center gap-3">
                <Icon
                  icon="mdi:account-group"
                  className="text-2xl text-stone-700"
                />
                <h3 className="font-serif text-xl text-stone-700">
                  Control who can access
                </h3>
              </div>
              <p className="text-sm leading-relaxed text-neutral-600">
                Invite selected people or teams to collaborate on notes with
                granular access controls.
              </p>
            </div>
            <CollaboratorsCell />
          </div>
          <div className="border-b border-neutral-100 bg-linear-to-b from-white to-stone-50/30">
            <div className="border-b border-neutral-100 p-4">
              <div className="mb-3 flex items-center gap-3">
                <Icon
                  icon="mdi:link-variant"
                  className="text-2xl text-stone-700"
                />
                <h3 className="font-serif text-xl text-stone-700">
                  Share instantly
                </h3>
              </div>
              <p className="text-sm leading-relaxed text-neutral-600">
                Send links or publish notes directly to Slack, Teams, or
                generate public shareable links.
              </p>
            </div>
            <ShareLinksCell />
          </div>
          <div className="bg-linear-to-b from-white to-stone-50/30">
            <div className="border-b border-neutral-100 p-4">
              <div className="mb-3 flex items-center gap-3">
                <Icon
                  icon="mdi:shield-lock"
                  className="text-2xl text-stone-700"
                />
                <h3 className="font-serif text-xl text-stone-700">
                  Track and protect
                </h3>
              </div>
              <p className="text-sm leading-relaxed text-neutral-600">
                DocSend-like features including view tracking, expiration dates,
                copy protection, and watermarks.
              </p>
            </div>
            <TrackProtectCell />
          </div>
        </div>
      </div>
    </section>
  );
}
const floatingPanelTabs = [
  {
    title: "Compact Mode",
    description:
      "Minimal overlay that indicates recording is active. Stays out of your way.",
    image: "/api/images/hyprnote/float-compact.jpg",
  },
  {
    title: "Memos",
    description:
      "Take quick notes during the meeting without losing focus on the conversation.",
    image: "/api/images/hyprnote/float-memos.jpg",
  },
  {
    title: "Transcript",
    description:
      "Watch the live transcript as the conversation unfolds in real-time.",
    image: "/api/images/hyprnote/float-transcript.jpg",
  },
  {
    title: "Live Insights",
    description:
      "Rolling summary of the past 5 minutes with AI suggestions and next steps.",
    image: "/api/images/hyprnote/float-insights.jpg",
  },
  {
    title: "Chat",
    description: "Ask questions and get instant answers during the meeting.",
    image: "/api/images/hyprnote/float-chat.jpg",
  },
];

const AUTO_ADVANCE_DURATION = 5000;

function FloatingPanelSection() {
  return (
    <section id="floating-panel" className="border-y border-neutral-100">
      <FloatingPanelHeader />
      <FloatingPanelContent />
    </section>
  );
}

function FloatingPanelHeader() {
  return (
    <div className="px-4 py-12 text-center lg:px-0">
      <div className="mb-4 inline-block rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 px-4 py-1.5 text-xs font-medium text-neutral-900 shadow-xs">
        Coming Soon
      </div>
      <h2 className="mb-4 font-serif text-3xl text-stone-700">
        Floating panel for meetings
      </h2>
      <p className="mx-auto max-w-3xl text-base text-neutral-600">
        A compact overlay that stays on top during meetings but won't show when
        you share your screen.
      </p>
    </div>
  );
}

function FloatingPanelContent() {
  const [selectedTab, setSelectedTab] = useState(0);
  const [progress, setProgress] = useState(0);
  const [isPaused, setIsPaused] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const progressRef = useRef(0);

  const handleTabIndexChange = useCallback((nextIndex: number) => {
    setSelectedTab(nextIndex);
    setProgress(0);
    progressRef.current = 0;
  }, []);

  useEffect(() => {
    if (isPaused) return;

    const startTime =
      Date.now() - (progressRef.current / 100) * AUTO_ADVANCE_DURATION;
    let animationId: number;

    const animate = () => {
      const elapsed = Date.now() - startTime;
      const newProgress = Math.min(
        (elapsed / AUTO_ADVANCE_DURATION) * 100,
        100,
      );
      setProgress(newProgress);
      progressRef.current = newProgress;

      if (newProgress >= 100) {
        const nextIndex = (selectedTab + 1) % floatingPanelTabs.length;
        setSelectedTab(nextIndex);
        setProgress(0);
        progressRef.current = 0;
        if (scrollRef.current) {
          const container = scrollRef.current;
          const scrollLeft = container.offsetWidth * nextIndex;
          container.scrollTo({
            left: scrollLeft,
            behavior: "smooth",
          });
        }
      } else {
        animationId = requestAnimationFrame(animate);
      }
    };

    animationId = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(animationId);
  }, [selectedTab, isPaused]);

  const scrollToTab = (index: number) => {
    setSelectedTab(index);
    setProgress(0);
    progressRef.current = 0;
    if (scrollRef.current) {
      const container = scrollRef.current;
      const scrollLeft = container.offsetWidth * index;
      container.scrollTo({ left: scrollLeft, behavior: "smooth" });
    }
  };

  const handleTabClick = (index: number) => {
    setSelectedTab(index);
    setProgress(0);
    progressRef.current = 0;
  };

  return (
    <div className="border-t border-neutral-100">
      <FloatingPanelMobile
        scrollRef={scrollRef}
        selectedTab={selectedTab}
        onIndexChange={handleTabIndexChange}
        scrollToTab={scrollToTab}
        progress={progress}
      />
      <FloatingPanelTablet
        selectedTab={selectedTab}
        progress={progress}
        onTabClick={handleTabClick}
        onPauseChange={setIsPaused}
      />
      <FloatingPanelDesktop />
    </div>
  );
}

function FloatingPanelTablet({
  selectedTab,
  progress,
  onTabClick,
  onPauseChange,
}: {
  selectedTab: number;
  progress: number;
  onTabClick: (index: number) => void;
  onPauseChange: (paused: boolean) => void;
}) {
  return (
    <div className="hidden border-t border-neutral-100 min-[800px]:max-[1000px]:block">
      <div className="flex flex-col">
        <div className="scrollbar-hide overflow-x-auto border-b border-neutral-100">
          <div className="flex">
            {floatingPanelTabs.map((tab, index) => (
              <button
                key={index}
                onClick={() => onTabClick(index)}
                onMouseEnter={() =>
                  selectedTab === index && onPauseChange(true)
                }
                onMouseLeave={() =>
                  selectedTab === index && onPauseChange(false)
                }
                className={cn([
                  "relative flex min-w-[280px] cursor-pointer flex-col items-start overflow-hidden border-r border-neutral-100 p-6 text-left transition-colors last:border-r-0",
                  selectedTab !== index && "hover:bg-neutral-50",
                ])}
              >
                {selectedTab === index && (
                  <div
                    className="absolute inset-0 bg-stone-100 transition-none"
                    style={{ width: `${progress}%` }}
                  />
                )}
                <div className="relative">
                  <h3 className="mb-1 font-serif text-base font-medium text-stone-700">
                    {tab.title}
                  </h3>
                  <p className="text-sm text-neutral-600">{tab.description}</p>
                </div>
              </button>
            ))}
          </div>
        </div>

        <div
          className="aspect-4/3"
          onMouseEnter={() => onPauseChange(true)}
          onMouseLeave={() => onPauseChange(false)}
        >
          <img
            src={floatingPanelTabs[selectedTab].image}
            alt={`${floatingPanelTabs[selectedTab].title} preview`}
            className="h-full w-full object-cover"
          />
        </div>
      </div>
    </div>
  );
}

function FloatingPanelDesktop() {
  const [selectedTab, setSelectedTab] = useState<number>(0);
  const [progress, setProgress] = useState(0);
  const [isPaused, setIsPaused] = useState(false);
  const progressRef = useRef(0);

  useEffect(() => {
    if (isPaused) return;

    const startTime =
      Date.now() - (progressRef.current / 100) * AUTO_ADVANCE_DURATION;
    let animationId: number;

    const animate = () => {
      const elapsed = Date.now() - startTime;
      const newProgress = Math.min(
        (elapsed / AUTO_ADVANCE_DURATION) * 100,
        100,
      );
      setProgress(newProgress);
      progressRef.current = newProgress;

      if (newProgress >= 100) {
        setSelectedTab((prev) => (prev + 1) % floatingPanelTabs.length);
        setProgress(0);
        progressRef.current = 0;
      } else {
        animationId = requestAnimationFrame(animate);
      }
    };

    animationId = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(animationId);
  }, [selectedTab, isPaused]);

  const handleTabClick = (index: number) => {
    setSelectedTab(index);
    setProgress(0);
    progressRef.current = 0;
  };

  return (
    <div className="hidden grid-cols-2 border-t border-neutral-100 min-[1000px]:grid">
      <div
        className="relative overflow-hidden border-r border-neutral-100"
        style={{ paddingBottom: "56.25%" }}
      >
        <div className="absolute inset-0 overflow-y-auto">
          {floatingPanelTabs.map((tab, index) => (
            <div
              key={index}
              onClick={() => handleTabClick(index)}
              onMouseEnter={() => selectedTab === index && setIsPaused(true)}
              onMouseLeave={() => selectedTab === index && setIsPaused(false)}
              className={cn([
                "relative cursor-pointer overflow-hidden p-6 transition-colors",
                index < tabs.length - 1 && "border-b border-neutral-100",
                selectedTab !== index && "hover:bg-neutral-50",
              ])}
            >
              {selectedTab === index && (
                <div
                  className="absolute inset-0 bg-stone-100 transition-none"
                  style={{ width: `${progress}%` }}
                />
              )}
              <div className="relative">
                <h3 className="mb-1 font-serif text-base font-medium text-stone-700">
                  {tab.title}
                </h3>
                <p className="text-sm text-neutral-600">{tab.description}</p>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div
        className="flex aspect-4/3 items-center justify-center overflow-hidden bg-neutral-100"
        onMouseEnter={() => setIsPaused(true)}
        onMouseLeave={() => setIsPaused(false)}
      >
        <img
          src={floatingPanelTabs[selectedTab].image}
          alt={`${floatingPanelTabs[selectedTab].title} preview`}
          className="h-full w-full object-cover"
        />
      </div>
    </div>
  );
}

function FloatingPanelMobile({
  scrollRef,
  selectedTab,
  onIndexChange,
  scrollToTab,
  progress,
}: {
  scrollRef: React.RefObject<HTMLDivElement | null>;
  selectedTab: number;
  onIndexChange: (index: number) => void;
  scrollToTab: (index: number) => void;
  progress: number;
}) {
  return (
    <div className="hidden max-[800px]:block">
      <div
        ref={scrollRef}
        className="scrollbar-hide snap-x snap-mandatory overflow-x-auto"
        onScroll={(e) => {
          const container = e.currentTarget;
          const scrollLeft = container.scrollLeft;
          const itemWidth = container.offsetWidth;
          const index = Math.round(scrollLeft / itemWidth);
          if (index !== selectedTab) {
            onIndexChange(index);
          }
        }}
      >
        <div className="flex">
          {floatingPanelTabs.map((tab, index) => (
            <div key={index} className="w-full shrink-0 snap-center">
              <div className="flex flex-col overflow-hidden border-y border-neutral-100">
                <div className="aspect-4/3 overflow-hidden border-b border-neutral-100">
                  <img
                    src={tab.image}
                    alt={`${tab.title} preview`}
                    className="h-full w-full object-cover"
                  />
                </div>
                <div className="p-6">
                  <p className="text-sm leading-relaxed text-neutral-600">
                    <span className="font-semibold text-stone-800">
                      {tab.title}
                    </span>{" "}
                    – {tab.description}
                  </p>
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="flex justify-center gap-2 py-6">
        {floatingPanelTabs.map((_, index) => (
          <button
            key={index}
            onClick={() => scrollToTab(index)}
            className={cn([
              "h-1 cursor-pointer overflow-hidden rounded-full",
              selectedTab === index
                ? "w-8 bg-neutral-300"
                : "w-8 bg-neutral-300 hover:bg-neutral-400",
            ])}
            aria-label={`Go to tab ${index + 1}`}
          >
            {selectedTab === index && (
              <div
                className="h-full bg-stone-600 transition-none"
                style={{ width: `${progress}%` }}
              />
            )}
          </button>
        ))}
      </div>
    </div>
  );
}

function CTASection() {
  return (
    <section className="bg-linear-to-t from-stone-50/30 to-stone-100/30 px-4 py-16 lg:px-0">
      <div className="flex flex-col items-center gap-6 text-center">
        <div className="mb-4 flex size-40 items-center justify-center rounded-[48px] border border-neutral-100 bg-transparent shadow-2xl">
          <img
            src="/api/images/hyprnote/icon.png"
            alt="Char"
            width={144}
            height={144}
            className="mx-auto size-36 rounded-[40px] border border-neutral-100"
          />
        </div>
        <h2 className="font-serif text-2xl sm:text-3xl">
          The complete AI notetaking solution
        </h2>
        <p className="mx-auto max-w-2xl text-lg text-neutral-600">
          From live meetings to archived recordings, handle all your audio
          transcription and AI summary needs with one powerful tool
        </p>
        <div className="pt-6">
          <Link
            to="/download/"
            className={cn([
              "group flex h-12 items-center justify-center px-6 text-base sm:text-lg",
              "rounded-full bg-linear-to-t from-stone-600 to-stone-500 text-white",
              "shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
              "transition-all",
            ])}
          >
            Download for free
            <svg
              xmlns="http://www.w3.org/2000/svg"
              fill="none"
              viewBox="0 0 24 24"
              strokeWidth="1.5"
              stroke="currentColor"
              className="ml-2 h-5 w-5 transition-transform group-hover:translate-x-1"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="m12.75 15 3-3m0 0-3-3m3 3h-7.5M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z"
              />
            </svg>
          </Link>
        </div>
      </div>
    </section>
  );
}
