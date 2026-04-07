import { Icon } from "@iconify-icon/react";
import MuxPlayer, { type MuxPlayerRefAttributes } from "@mux/mux-player-react";
import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { allArticles } from "content-collections";
import { AnimatePresence, motion, useInView } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";

import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { cn } from "@hypr/utils";

import { AcquisitionLinkGrid } from "@/components/acquisition-link-grid";
import {
  ContactSearchToolCall,
  TranscriptToolCall,
} from "@/components/ai-feature-panel";
import { AppPreviewSection } from "@/components/app-preview";
import { CTASection } from "@/components/cta-section";
import { DownloadButton } from "@/components/download-button";
import { GitHubOpenSource } from "@/components/github-open-source";
import { GithubStars } from "@/components/github-stars";
import { Image } from "@/components/image";
import { LogoCloud } from "@/components/logo-cloud";
import { FAQ, FAQItem } from "@/components/mdx-shared";
import { NotebookGrid } from "@/components/notebook-grid";
import { SocialCard } from "@/components/social-card";
import { VideoModal } from "@/components/video-modal";
import { addContact } from "@/functions/loops";
import { useHeroContext } from "@/hooks/use-hero-context";
import { getHeroCTA, usePlatform } from "@/hooks/use-platform";
import { useAnalytics } from "@/hooks/use-posthog";
import {
  CHAR_SITE_URL,
  ROOT_DESCRIPTION,
  getOrganizationJsonLd,
  getSoftwareApplicationJsonLd,
  getStructuredDataGraph,
} from "@/lib/seo";

const MUX_PLAYBACK_ID = "1s01BC9LBwzygOUWk9Pdn011KuxvIQRMbTEfCpOypfdrw";

const mainFeatures = [
  {
    icon: "mdi:text-box-outline",
    title: "Real-time transcription",
    description:
      "While you take notes, Char listens and generates a live transcript",
    image: "/api/images/hyprnote/transcript.jpg",
    muxPlaybackId: "rbkYuZpGJGLHx023foq9DCSt3pY1RegJU5PvMCkRE3rE",
    link: "/product/ai-notetaking/#transcription",
  },
  {
    icon: "mdi:file-document-outline",
    title: "AI summary",
    description:
      "Char combines your notes and the transcript to create a perfect summary",
    image: "/api/images/hyprnote/summary.jpg",
    muxPlaybackId: "lKr5l1fWGNnRqOehiz15mV79VHtFOCiuO9urmgqs6V8",
    link: "/product/ai-notetaking/#summaries",
  },
  {
    icon: "mdi:chat-outline",
    title: "AI Chat",
    description:
      "Use natural language to get answers pulled directly from your transcript",
    image: "/api/images/hyprnote/chat.jpg",
    link: "/product/ai-assistant",
  },
  {
    icon: "mdi:window-restore",
    title: "Floating panel",
    description: "Overlay to quick access recording controls during calls",
    image: "/api/images/hyprnote/floating.jpg",
    link: "/product/ai-notetaking/#floating-panel",
  },
  {
    icon: "mdi:keyboard-outline",
    title: "Keyboard shortcuts",
    description: "Navigate and format quickly without touching your mouse",
    image: "/api/images/hyprnote/editor.jpg",
    muxPlaybackId: "sMWkuSxKWfH3RYnX51Xa2acih01ZP5yfQy01Q00XRd1yTQ",
    link: "/docs/faq/keyboard-shortcuts",
  },
];

const activeFeatureIndices = mainFeatures.map((_, i) => i);
const FEATURES_AUTO_ADVANCE_DURATION = 8000;

export const Route = createFileRoute("/_view/")({
  component: Component,
  head: () => ({
    links: [{ rel: "canonical", href: CHAR_SITE_URL }],
    scripts: [
      {
        type: "application/ld+json",
        children: JSON.stringify(
          getStructuredDataGraph([
            getOrganizationJsonLd(),
            getSoftwareApplicationJsonLd({
              description: ROOT_DESCRIPTION,
              featureList: [
                "Bot-free meeting capture",
                "Local transcription",
                "Bring your own AI keys",
                "Markdown files you own",
                "Optional cloud AI and sync",
              ],
            }),
          ]),
        ),
      },
    ],
  }),
});

function useHasEnteredView<T extends Element>(
  amount: "some" | "all" | number = 0.2,
) {
  const ref = useRef<T>(null);
  const isInView = useInView(ref, { amount });
  const [hasEnteredView, setHasEnteredView] = useState(false);

  useEffect(() => {
    if (isInView) {
      setHasEnteredView(true);
    }
  }, [isInView]);

  return { ref, isInView, hasEnteredView };
}

function Component() {
  const [expandedVideo, setExpandedVideo] = useState<string | null>(null);
  const heroInputRef = useRef<HTMLInputElement>(null);

  return (
    <main className="min-h-screen flex-1 overflow-x-hidden px-2 md:px-8">
      <div className="">
        {/* <AnnouncementBanner /> */}
        <HeroSection
          onVideoExpand={setExpandedVideo}
          heroInputRef={heroInputRef}
        />
        <LogoSection />
        <HowItWorksSection />
        <AppPreviewSection />
        <AISection />
        <GrowsWithYouSection />
        <SolutionsTabbar />
        <ExplorePathsSection />
        <SocialTestimonialsSection />
        <GitHubOpenSource />
        <FAQSection />
        <BlogSection />
        <CTASection heroInputRef={heroInputRef} />
      </div>
      <VideoModal
        playbackId={expandedVideo || ""}
        isOpen={expandedVideo !== null}
        onClose={() => setExpandedVideo(null)}
      />
    </main>
  );
}

function HeroSection({
  onVideoExpand,
  heroInputRef,
}: {
  onVideoExpand: (id: string) => void;
  heroInputRef: React.RefObject<HTMLInputElement | null>;
}) {
  const platform = usePlatform();
  const heroCTA = getHeroCTA(platform);
  const heroContext = useHeroContext();
  const { track } = useAnalytics();
  const [shake, setShake] = useState(false);

  useEffect(() => {
    track("hero_section_viewed", {
      timestamp: new Date().toISOString(),
    });
  }, [track]);

  const mutation = useMutation({
    mutationFn: async (email: string) => {
      const intent = platform === "mobile" ? "Reminder" : "Waitlist";
      const eventName =
        platform === "mobile" ? "reminder_requested" : "os_waitlist_joined";

      track(eventName, {
        platform: platform,
        timestamp: new Date().toISOString(),
        email: email,
      });

      await addContact({
        data: {
          email,
          userGroup: "Lead",
          platform:
            platform === "mobile"
              ? "Mobile"
              : platform.charAt(0).toUpperCase() + platform.slice(1),
          source: "LANDING_PAGE",
          intent: intent,
        },
      });
    },
  });

  const form = useForm({
    defaultValues: {
      email: "",
    },
    onSubmit: async ({ value }) => {
      await mutation.mutateAsync(value.email);
      form.reset();
    },
  });

  const handleTrigger = useCallback(() => {
    const inputEl = heroInputRef.current;
    if (inputEl) {
      inputEl.focus();
      setShake(true);
      setTimeout(() => setShake(false), 500);
    }
  }, []);

  useEffect(() => {
    if (heroContext) {
      heroContext.setOnTrigger(handleTrigger);
    }
  }, [heroContext, handleTrigger]);

  return (
    <div className="">
      <div className="flex w-full flex-col text-left">
        <section
          id="hero"
          className="isolate flex w-full overflow-visible pt-10 text-left"
        >
          <div className="border-brand-bright items-left relative z-10 flex min-h-[80vh] w-full flex-col content-between rounded-lg border md:flex-row">
            <div className="flex flex-col justify-between px-4 pt-8 pb-8 md:px-6 md:pt-12 md:pr-8 md:pb-12 md:pl-12">
              <div className="flex flex-col gap-2">
                <h1
                  className="text-color break-words"
                  style={{
                    fontSize: "clamp(1.5rem, 2rem + 3.2vw, 3.75rem)",
                  }}
                >
                  Meeting Notes <br /> You Own
                </h1>
                <p className="font-regular text-fg-muted text-base leading-relaxed break-words sm:text-xl">
                  Char captures every meeting without a bot and keeps data on
                  your device.
                </p>
                {heroCTA.showInput ? (
                  <form
                    onSubmit={(e) => {
                      e.preventDefault();
                      form.handleSubmit();
                    }}
                    className="w-full max-w-md text-left"
                  >
                    <form.Field
                      name="email"
                      validators={{
                        onChange: ({ value }) => {
                          if (!value) {
                            return "Email is required";
                          }
                          if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value)) {
                            return "Please enter a valid email";
                          }
                          return undefined;
                        },
                      }}
                    >
                      {(field) => (
                        <>
                          <div
                            className={cn([
                              "items-left relative flex overflow-hidden rounded-full border-2 transition-all duration-200",
                              shake && "animate-shake border-stone-600",
                              !shake && mutation.isError && "border-red-500",
                              !shake &&
                                mutation.isSuccess &&
                                "border-green-500",
                              !shake &&
                                !mutation.isError &&
                                !mutation.isSuccess &&
                                "border-neutral-200 focus-within:border-stone-500",
                            ])}
                          >
                            <input
                              ref={heroInputRef}
                              type="email"
                              value={field.state.value}
                              onChange={(e) =>
                                field.handleChange(e.target.value)
                              }
                              onBlur={field.handleBlur}
                              placeholder={heroCTA.inputPlaceholder}
                              className="flex-1 bg-white px-6 py-4 text-base outline-hidden"
                              disabled={
                                mutation.isPending || mutation.isSuccess
                              }
                            />
                            <button
                              type="submit"
                              disabled={
                                mutation.isPending || mutation.isSuccess
                              }
                              className="absolute top-1 right-1 bottom-1 rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-4 text-sm text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%] disabled:opacity-50 sm:px-6"
                            >
                              {mutation.isPending
                                ? "Sending..."
                                : mutation.isSuccess
                                  ? "Sent!"
                                  : heroCTA.buttonLabel}
                            </button>
                          </div>
                          {mutation.isSuccess && (
                            <p className="mt-4 text-sm text-green-600">
                              Thanks! We'll be in touch soon.
                            </p>
                          )}
                          {mutation.isError && (
                            <p className="mt-4 text-sm text-red-600">
                              {mutation.error instanceof Error
                                ? mutation.error.message
                                : "Something went wrong. Please try again."}
                            </p>
                          )}
                          {!mutation.isSuccess &&
                            !mutation.isError &&
                            (heroCTA.subtextLink ? (
                              <Link
                                to={heroCTA.subtextLink}
                                target="_blank"
                                rel="noopener noreferrer"
                                className="text-fg-muted hover:text-color mt-4 block text-sm decoration-dotted transition-colors hover:underline"
                              >
                                {heroCTA.subtext}
                              </Link>
                            ) : (
                              <p className="text-fg-muted mt-4 text-sm">
                                {heroCTA.subtext}
                              </p>
                            ))}
                        </>
                      )}
                    </form.Field>
                  </form>
                ) : (
                  <div className="mt-4 flex w-full flex-col items-stretch gap-4 lg:flex-row lg:items-start">
                    <DownloadButton />
                    <GithubStars />
                  </div>
                )}
              </div>
            </div>

            <div className="relative hidden w-full shrink-0 self-stretch overflow-hidden p-8 md:block md:w-1/2">
              <NotebookGrid />

              <div className="absolute right-0 bottom-0 flex justify-end p-10">
                <button
                  onClick={() => onVideoExpand(MUX_PLAYBACK_ID)}
                  className="group surface border-color-brand relative flex w-4/5 flex-col overflow-hidden rounded-xl border shadow-xl"
                  style={{ aspectRatio: "16/9" }}
                >
                  <div className="h-full w-full">
                    <img
                      src="/demo_thumbnail.webp"
                      alt="Product demo"
                      className="h-full w-full object-cover"
                    />
                    <div className="absolute inset-0 flex items-center justify-center transition-colors group-hover:bg-black/30">
                      <div className="flex size-10 items-center justify-center rounded-full bg-white/90 shadow-lg transition-transform group-hover:scale-110">
                        <Icon
                          icon="mdi:play"
                          className="text-color ml-0.5 text-lg"
                        />
                      </div>
                    </div>
                  </div>
                </button>
              </div>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}

function LogoSection() {
  return (
    <section className="px-4 py-12 md:py-24">
      <h3 className="text-fg mb-4 font-mono text-xs font-medium tracking-widest uppercase">
        Trusted by people in:
      </h3>
      <LogoCloud />
    </section>
  );
}

function SocialTestimonialsSection() {
  return (
    <section className="px-4 pt-16 pb-16">
      <h2 className="text-color border-color-brand mb-10 border-b pb-8 font-mono text-2xl tracking-wide md:text-4xl">
        <span className="mb-2 block">What people are saying</span>
        <span className="text-fg-muted block font-sans text-sm font-normal tracking-normal md:text-base">
          Char was formerly Hyprnote.{" "}
          <Link
            to="/blog/$slug/"
            params={{ slug: "hyprnote-is-now-char" }}
            className="text-color underline underline-offset-4 transition-opacity hover:opacity-70"
          >
            Read about the rename.
          </Link>
        </span>
      </h2>

      <div className="flex flex-col gap-6 md:hidden">
        <SocialCard
          platform="twitter"
          author="yoran was here"
          username="yoran_beisher"
          body="Been using Hypernote for a while now, truly one of the best AI apps I've used all year. Like they said, the best thing since sliced bread"
          url="https://x.com/yoran_beisher/status/1953147865486012611"
        />
        <SocialCard
          platform="twitter"
          author="Tom Yang"
          username="tomyang11_"
          avatar="/avatars/tom.jpg"
          body="I love the flexibility that @tryhyprnote gives me to integrate personal notes with AI summaries. I can quickly jot down important points during the meeting without getting distracted, then trust that the AI will capture them in full detail for review afterwards."
          url="https://twitter.com/tomyang11_/status/1956395933538902092"
        />
      </div>

      <div className="hidden gap-4 md:grid md:grid-cols-3">
        <SocialCard
          platform="twitter"
          author="Tobi Lutke"
          username="tobi"
          avatar="/avatars/tobi.jpg"
          body={`I'm actually very pro meeting recording and ai summarization. But I'm not ok with bots joining as fake humans accomplish this. It's a meeting between you and me. Not you and me and some startup's viral growth strategy.

Granola is great. Gemini does this well in Google Meet. Hyprnote is great and fully local. But use them with consent.

My tweet is about how ridiculous and self important it looks when you show up to a meeting with random bots as entourage.`}
          url="https://x.com/tobi/status/1983892259230699921"
        />
        <SocialCard
          platform="twitter"
          author="Anand Chowdhary"
          username="AnandChowdhary"
          avatar="/avatars/anand.jpg"
          body={`Hyprnote has been on my radar since their time in YC S25 as “that local-first meeting notes thing,” and I finally took a closer look today. It immediately hit a nerve I’ve had with AI note tools for years. I love the idea of getting help with meetings. I really don’t love bots joining every Zoom call or my audio being streamed to some mystery server “for quality purposes”.

@tryhyprnote leans into that tension in a pretty honest way. It calls itself a local-first AI notepad for private meetings, and the “private” bit is not just a tagline. There are no meeting bots and no calendar guests. It just listens directly to the audio going in and out of your computer, gives you a realtime transcript, and lets you stay in the conversation instead of turning into a court reporter.

You still have a simple notepad to jot quick memos during the call. Those act more like hints than homework. After the meeting, Hyprnote can use your memos to shape a personalized summary, but that part is optional. If you forget to take notes altogether, it can still generate a recap from the transcript.

The tech stack is pretty nice if you are into that sort of thing. TypeScript and React on the UI, Rust and Tauri for the desktop app. The cool part is what that enables. You can run the whole thing offline with LM Studio or Ollama. No Wi‑Fi, no outbound requests. That makes it genuinely interesting for teams that care a lot about compliance or even air‑gapped environments. And if you do want cloud models, it does the “bring your own LLM” thing with Gemini, Claude, Azure‑hosted GPT, etc., so it can fit into whatever your company’s approved stack is.

If you have been waiting for an AI meeting assistant that behaves like a real desktop app and respects the fact that you might not want to ship your raw meeting audio to the cloud, Hyprnote is worth a look`}
          url="https://x.com/AnandChowdhary/status/1997980479698723119"
        />
        <div className="flex flex-col gap-8">
          <SocialCard
            platform="twitter"
            author="James Koshigoe"
            username="JamesKoshigoe"
            avatar="/avatars/james.jpg"
            body={`@getcharnotes
 is by far one of my favorite AI secret weapons as of late. It's an AI notetaking tool, and there's a ton, but it's the best open source one that respects privacy & isn't a walled garden like others
No affiliation, just love their product & hope they succeed`}
            url="https://x.com/JamesKoshigoe/status/2024676687980671195"
          />
          <SocialCard
            platform="twitter"
            author="Tom Yang"
            username="tomyang11_"
            avatar="/avatars/tom.jpg"
            body="I love the flexibility that @tryhyprnote gives me to integrate personal notes with AI summaries. I can quickly jot down important points during the meeting without getting distracted, then trust that the AI will capture them in full detail for review afterwards."
            url="https://twitter.com/tomyang11_/status/1956395933538902092"
          />
        </div>
      </div>
    </section>
  );
}

const DOT_SPACING = 8;
const DOT_RADIUS = 1.2;
const WAVE_PATH =
  "M44.665 0.5C60.7718 0.500161 75.5325 8.93172 88.1582 19.5205C106.895 35.2347 130.869 44.7871 157 44.7871C183.131 44.7871 207.103 35.2338 225.84 19.5195C238.465 8.93064 253.226 0.500001 269.333 0.5H313.5V52.4854H261.956C244.715 52.4854 228.565 61.2064 218.681 75.8398L212.83 84.5H99.7422L93.8926 75.8398C84.008 61.2063 67.8572 52.4854 50.6162 52.4854H0.5V0.5H44.665Z";

function DotWaveTransition() {
  const dots: { cx: number; cy: number; delay: number }[] = [];
  const padding = DOT_SPACING / 2;
  const rows = Math.floor((85 - padding * 2) / DOT_SPACING);
  const cols = Math.floor((314 - padding * 2) / DOT_SPACING);

  for (let r = 0; r <= rows; r++) {
    for (let c = 0; c <= cols; c++) {
      dots.push({
        cx: padding + c * DOT_SPACING,
        cy: padding + r * DOT_SPACING,
        delay: (r / rows) * 3,
      });
    }
  }

  return (
    <svg
      className="text-fg-subtle"
      width="100%"
      height="100%"
      viewBox="0 0 314 85"
      xmlns="http://www.w3.org/2000/svg"
    >
      <defs>
        <clipPath id="wave-clip">
          <path d={WAVE_PATH} />
        </clipPath>
      </defs>
      <path d={WAVE_PATH} fill="none" stroke="" />
      <g clipPath="url(#wave-clip)">
        {dots.map((dot, i) => (
          <circle
            key={i}
            cx={dot.cx}
            cy={dot.cy}
            r={DOT_RADIUS}
            fill="var(--color-fg)"
            className="animate-dot-wave"
            style={{ animationDelay: `${dot.delay}s` }}
          />
        ))}
      </g>
    </svg>
  );
}

export function HowItWorksSection() {
  const [enhancedLines, setEnhancedLines] = useState(0);
  const { ref, isInView } = useHasEnteredView<HTMLElement>(0.2);
  const featureScrollRef = useRef<HTMLDivElement>(null);
  const [showLeftGrad, setShowLeftGrad] = useState(false);
  const [showRightGrad, setShowRightGrad] = useState(true);

  const handleFeatureScroll = useCallback(() => {
    const el = featureScrollRef.current;
    if (!el) return;
    setShowLeftGrad(el.scrollLeft > 8);
    setShowRightGrad(el.scrollLeft + el.offsetWidth < el.scrollWidth - 8);
  }, []);

  useEffect(() => {
    if (!isInView) {
      return;
    }

    const timeouts: ReturnType<typeof setTimeout>[] = [];

    const runAnimation = () => {
      setEnhancedLines(0);

      timeouts.push(
        setTimeout(() => {
          setEnhancedLines(1);
          timeouts.push(
            setTimeout(() => {
              setEnhancedLines(2);
              timeouts.push(
                setTimeout(() => {
                  setEnhancedLines(3);
                  timeouts.push(
                    setTimeout(() => {
                      setEnhancedLines(4);
                      timeouts.push(
                        setTimeout(() => {
                          setEnhancedLines(5);
                          timeouts.push(
                            setTimeout(() => {
                              setEnhancedLines(6);
                              timeouts.push(
                                setTimeout(() => {
                                  setEnhancedLines(7);
                                  timeouts.push(
                                    // animation stops after last line
                                  );
                                }, 800),
                              );
                            }, 800),
                          );
                        }, 800),
                      );
                    }, 800),
                  );
                }, 800),
              );
            }, 800),
          );
        }, 800),
      );
    };

    runAnimation();
    return () => {
      timeouts.forEach(clearTimeout);
    };
  }, [isInView]);

  return (
    <section ref={ref} id="how-it-works" className="px-4 pt-8 pb-12 md:pb-24">
      <div className="flex flex-col">
        {/* Header */}
        <div className="border-color-brand border-b py-10">
          <h2 className="text-color font-mono text-2xl leading-relaxed tracking-wide md:text-5xl">
            Focus on conversation <br /> while Char makes notes
          </h2>
        </div>

        {/* Block 1: Listen & Write */}
        <div className="flex flex-col md:flex-row">
          <div className="flex flex-col justify-end gap-4 py-8 md:w-1/2 md:pr-8 md:pb-16">
            <p className="text-fg font-mono text-xs tracking-widest uppercase opacity-50">
              During meeting
            </p>
            <p className="font-regular text-color text-lg leading-relaxed md:text-2xl lg:text-3xl">
              Char keeps track of everything that happens during the meeting,
              includes context about previous conversations and people you talk
              to.
            </p>
          </div>

          <div className="bg-lined-notebook select-none md:w-1/2">
            <motion.div
              initial="hidden"
              whileInView="visible"
              viewport={{ once: true, amount: 0.3 }}
              variants={{
                hidden: {},
                visible: { transition: { staggerChildren: 0.3 } },
              }}
              className="flex flex-col gap-4 p-4 lg:p-8"
            >
              <motion.div
                variants={{
                  hidden: { opacity: 0, y: -15 },
                  visible: {
                    opacity: 1,
                    y: 0,
                    transition: { duration: 0.5, ease: "easeOut" },
                  },
                }}
                className="flex h-14 w-full items-center justify-between rounded-full bg-stone-700 p-2 pl-6 md:h-20 md:pl-8"
              >
                <div className="flex items-center gap-3">
                  <div className="relative flex size-3">
                    <span
                      className={cn([
                        "absolute inline-flex size-full rounded-full bg-red-400 opacity-75",
                        isInView && "animate-ping",
                      ])}
                    />
                    <span className="relative inline-flex size-3 rounded-full bg-red-500" />
                  </div>
                  <p className="text-sm text-white md:text-base">
                    Meeting in progress...
                  </p>
                </div>
                <div className="flex items-center gap-1 md:gap-2">
                  <div className="flex h-full items-center justify-center rounded-full px-2 md:px-3">
                    <Icon
                      icon="mdi:dots-horizontal"
                      className="text-xl text-white/60 md:text-2xl"
                    />
                  </div>
                  <div className="flex size-10 items-center justify-center rounded-full bg-red-600 md:h-full md:w-[72px] md:py-3">
                    <Icon
                      icon="mdi:phone-hangup"
                      className="text-2xl text-white md:text-4xl"
                    />
                  </div>
                </div>
              </motion.div>
              <div className="flex flex-col gap-4 sm:flex-row">
                {/* Notes panel */}
                <motion.div
                  variants={{
                    hidden: { opacity: 0, y: -15 },
                    visible: {
                      opacity: 1,
                      y: 0,
                      transition: { duration: 0.5, ease: "easeOut" },
                    },
                  }}
                  className="border-color-brand bg-surface h-[200px] w-full overflow-hidden rounded-xl border sm:h-[300px] sm:w-1/2"
                >
                  <div className="border-color-brand bg-surface-subtle relative flex h-[38px] shrink-0 items-center gap-2 border-b px-4">
                    <div className="flex gap-2">
                      <div className="size-3 rounded-full bg-red-400" />
                      <div className="size-3 rounded-full bg-yellow-400" />
                      <div className="size-3 rounded-full bg-green-400" />
                    </div>
                    <div className="absolute left-1/2 -translate-x-1/2">
                      <span className="text-fg-muted font-mono text-sm font-medium">
                        Char
                      </span>
                    </div>
                  </div>

                  <div className="overflow-auto p-4">
                    <h4 className="text-color mb-2 text-sm font-semibold">
                      Active meeting
                    </h4>
                    <div className="text-color overflow-hidden text-sm whitespace-pre-line lg:text-base">
                      {"ui update - moble\napi\nnew dash - urgnet"}
                      <motion.span
                        className="text-2xl text-blue-600"
                        animate={
                          isInView ? { opacity: [1, 0, 1] } : { opacity: 1 }
                        }
                        transition={{
                          duration: 0.8,
                          repeat: isInView ? Infinity : 0,
                          ease: "linear",
                        }}
                      >
                        |
                      </motion.span>
                    </div>
                  </div>
                </motion.div>
                <motion.div
                  variants={{
                    hidden: {},
                    visible: { transition: { staggerChildren: 0.15 } },
                  }}
                  className="grid w-full grid-cols-2 place-content-around sm:w-1/2 md:gap-2 xl:gap-4"
                >
                  {["design weekly.md", "1:1 with John.md", "Q2 goals.md"].map(
                    (name, i) => (
                      <motion.div
                        key={name}
                        variants={{
                          hidden: {
                            opacity: 0,
                            y: -10,
                            rotate: [-6, 6, -4][i],
                          },
                          visible: {
                            opacity: 1,
                            y: 0,
                            rotate: [-6, 6, -4][i],
                            transition: { duration: 0.4, ease: "easeOut" },
                          },
                        }}
                        className="bg-surface border-color-brand relative flex h-32 w-full flex-col items-end justify-end rounded border p-2"
                        style={{
                          clipPath:
                            "polygon(0 0, calc(100% - 24px) 0, 100% 24px, 100% 100%, 0 100%)",
                        }}
                      >
                        <div className="border-color-brand absolute top-0 right-0 h-[24px] w-[24px] bg-[var(--color-border)]" />
                        <p className="text-fg text-xs lg:text-sm">{name}</p>
                      </motion.div>
                    ),
                  )}
                  <motion.div
                    variants={{
                      hidden: { opacity: 0, y: -10 },
                      visible: {
                        opacity: 1,
                        y: 0,
                        transition: { duration: 0.4, ease: "easeOut" },
                      },
                    }}
                    className="flex flex-col justify-between"
                  >
                    {[
                      {
                        name: "Ben",
                        color: "bg-red-200 border-red-300 text-red-500",
                      },
                      {
                        name: "Sarah",
                        color: "bg-blue-200 border-blue-300 text-blue-500",
                      },
                      {
                        name: "Victor",
                        color: "bg-amber-200 border-amber-300 text-amber-500",
                      },
                    ].map(({ name, color }) => (
                      <div
                        key={name}
                        className="bg-surface flex items-center gap-2 rounded-full border border-stone-200 py-2 pr-4 pl-2"
                      >
                        <div
                          className={cn([
                            "flex size-5 min-w-5 items-center justify-center rounded-full border text-sm font-bold",
                            color,
                          ])}
                        >
                          {name[0]}
                        </div>
                        <span className="text-fg-muted text-sm font-medium">
                          {name}
                        </span>
                      </div>
                    ))}
                  </motion.div>
                </motion.div>
              </div>
            </motion.div>
          </div>
        </div>

        <div className="flex flex-col md:flex-row">
          <div className="md:w-1/2"></div>
          <div className="bg-lined-notebook flex flex-col justify-center px-4 py-8 select-none md:w-1/2 md:px-8 md:py-0">
            <DotWaveTransition />
          </div>
        </div>

        {/* Block 2: Summarize */}
        <div className="-mt-px flex flex-col md:flex-row">
          <div className="flex flex-col justify-start gap-4 py-8 md:w-1/2 md:pt-16 md:pr-8">
            <p className="text-fg font-mono text-xs tracking-widest uppercase opacity-50">
              After meeting
            </p>
            <p className="font-regular text-color text-lg leading-relaxed md:text-2xl lg:text-3xl">
              After the meeting, Char combines your notes with transcripts to
              create a perfect summary.
            </p>
          </div>

          <div className="bg-lined-notebook flex-1 select-none">
            <motion.div
              initial={{ opacity: 0, y: -20 }}
              whileInView={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.6, ease: "easeOut" }}
              viewport={{ once: true, amount: 0.6 }}
              className="flex h-full items-end justify-center p-4 lg:p-8"
            >
              <div className="surface border-color-brand relative max-h-[500px] w-full overflow-hidden rounded-xl border lg:max-h-none">
                <div className="pointer-events-none absolute inset-x-0 bottom-0 z-10 h-24 bg-gradient-to-t from-white to-transparent lg:hidden" />
                <div className="border-color-brand bg-surface-subtle relative flex h-[38px] items-center gap-2 border-b px-4">
                  <div className="flex gap-2">
                    <div className="size-3 rounded-full bg-red-400" />
                    <div className="size-3 rounded-full bg-yellow-400" />
                    <div className="size-3 rounded-full bg-green-400" />
                  </div>
                  <div className="absolute left-1/2 -translate-x-1/2">
                    <span className="text-fg-muted font-mono text-sm font-medium">
                      Char
                    </span>
                  </div>
                </div>
                <div className="relative flex w-full flex-col gap-4 overflow-hidden p-4 text-sm lg:p-6 lg:text-base">
                  <div className="flex flex-col gap-2">
                    <h4
                      className={cn([
                        "text-color text-base font-semibold transition-opacity duration-500 lg:text-lg",
                        enhancedLines >= 1 ? "opacity-100" : "opacity-0",
                      ])}
                    >
                      Mobile UI Update and API Adjustments
                    </h4>
                    <ul className="text-color flex list-disc flex-col gap-2 pl-5">
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
                        "text-color font-semibold transition-opacity duration-500",
                        enhancedLines >= 5 ? "opacity-100" : "opacity-0",
                      ])}
                    >
                      New Dashboard – Urgent Priority
                    </h4>
                    <ul className="text-color flex list-disc flex-col gap-2 pl-5">
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
                  <div className="pointer-events-none absolute right-0 bottom-0 left-0 h-56 bg-gradient-to-t from-white to-transparent" />
                </div>
              </div>
            </motion.div>
          </div>
        </div>

        {/* features block */}
        <div className="border-color-brand relative border-t">
          <div
            className={cn([
              "from-page pointer-events-none absolute top-0 bottom-0 left-0 z-10 w-8 bg-gradient-to-r to-transparent transition-opacity md:hidden",
              showLeftGrad ? "opacity-100" : "opacity-0",
            ])}
          />
          <div
            className={cn([
              "from-page pointer-events-none absolute top-0 right-0 bottom-0 z-10 w-8 bg-gradient-to-l to-transparent transition-opacity md:hidden",
              showRightGrad ? "opacity-100" : "opacity-0",
            ])}
          />
          <div
            ref={featureScrollRef}
            onScroll={handleFeatureScroll}
            className="flex snap-x snap-mandatory gap-4 overflow-x-auto pt-6 pb-4 [scrollbar-width:none] md:grid md:grid-cols-3 md:gap-8 md:overflow-visible md:pt-16 md:pb-0 md:*:min-h-[320px] md:*:py-4"
          >
            {/* own your data */}
            <div className="flex w-[85%] shrink-0 snap-start flex-col gap-2 md:w-auto md:shrink">
              <div className="flex h-32 items-center justify-start gap-2 select-none md:h-24 lg:h-32">
                <img
                  src="/icons/file.webp"
                  alt=""
                  className="w-10 rotate-[3deg] object-contain md:w-7 lg:w-10"
                  draggable={false}
                />
                <img
                  src="/icons/file.webp"
                  alt=""
                  className="w-10 rotate-[-5deg] object-contain md:w-7 lg:w-10"
                  draggable={false}
                />
                <img
                  src="/icons/folderchar.svg"
                  alt=""
                  className="w-14 object-contain md:w-10 lg:w-14"
                  draggable={false}
                />
                <img
                  src="/icons/file.webp"
                  alt=""
                  className="w-10 rotate-[6deg] object-contain md:w-7 lg:w-10"
                  draggable={false}
                />
                <img
                  src="/icons/file.webp"
                  alt=""
                  className="w-10 rotate-[-4deg] object-contain md:w-7 lg:w-10"
                  draggable={false}
                />
              </div>
              <div className="flex min-h-0 flex-col justify-start gap-2 md:max-h-[200px]">
                <h4 className="text-color mb-4 font-mono text-base font-medium md:text-xl">
                  Own your <br /> data
                </h4>
                <p className="text-color text-base">
                  Data stays on your device and not locked in a cloud.
                </p>
              </div>
            </div>

            {/* local or cloud */}
            <div className="flex w-[85%] shrink-0 snap-start flex-col gap-2 md:w-auto md:shrink">
              <div className="flex h-32 items-center gap-4 select-none md:h-24 md:gap-3 lg:h-32 lg:gap-4">
                <Icon
                  icon="mdi:wifi-off"
                  className="text-fg-muted text-2xl md:text-xl lg:text-2xl"
                />
                <div className="flex rounded-md border border-red-300 bg-red-100 px-2 py-2 md:px-1.5 md:py-1.5 lg:px-2 lg:py-2">
                  <DancingSticks
                    amplitude={1}
                    height={24}
                    width={100}
                    color="#ef4444"
                  />
                </div>
              </div>
              <div className="flex min-h-0 flex-col justify-start gap-2 md:max-h-[200px]">
                <h4 className="text-color mb-4 font-mono text-base font-medium md:text-xl">
                  Local or cloud,
                  <br /> your choice
                </h4>
                <p className="text-color text-base">
                  Run fully local, bring your own API key, or use Char cloud.
                  Switch anytime.
                </p>
              </div>
            </div>

            {/* works everywhere */}
            <div className="flex w-[85%] shrink-0 snap-start flex-col gap-2 md:w-auto md:shrink">
              <div className="flex h-32 items-center select-none md:h-24 lg:h-32">
                <div className="flex flex-wrap items-center gap-6 md:gap-4 lg:gap-6">
                  <img
                    src="/icons/zoom.svg"
                    alt="Zoom"
                    className="size-10 md:size-7 lg:size-10"
                    draggable={false}
                  />
                  <img
                    src="/icons/teams logo.svg"
                    alt="Teams"
                    className="size-12 md:size-8 lg:size-12"
                    draggable={false}
                  />
                  <img
                    src="/icons/google-meet logo.svg"
                    alt="Google Meet"
                    className="size-12 md:size-8 lg:size-12"
                    draggable={false}
                  />
                  <img
                    src="/icons/inperson logo.svg"
                    alt="In-person"
                    className="size-14 md:size-10 lg:size-14"
                    draggable={false}
                  />
                </div>
              </div>
              <div className="flex min-h-0 flex-col justify-start gap-2 md:max-h-[200px]">
                <h4 className="text-color mb-4 font-mono text-base font-medium md:text-xl">
                  Works <br /> everywhere
                </h4>
                <p className="text-color text-base">
                  Captures system audio. Works on Zoom, Teams, Meet, in-person,
                  or offline.
                </p>
              </div>
            </div>

            {/* upload existing recordings */}
            <div className="border-brand flex w-[85%] shrink-0 snap-start flex-col gap-2 md:w-auto md:shrink">
              <div className="flex h-32 items-center select-none md:h-24 lg:h-32">
                <div className="relative flex h-16 w-4/5 items-center justify-center rounded-lg border-2 border-dashed border-green-300 bg-green-100 px-2 py-2 md:h-12 md:w-full lg:h-16 lg:w-4/5">
                  <div className="flex size-10 items-center justify-center rounded-full bg-white md:size-7 lg:size-10">
                    <Icon
                      icon="mdi:file-upload"
                      className="text-fg-muted text-xl"
                    />
                  </div>
                  <div className="border-color-brand surface absolute flex rotate-8 flex-row items-center gap-2 rounded-md border py-3 pr-4 pl-2 text-nowrap shadow-lg lg:right-1/4 lg:bottom-1/4 lg:translate-x-[5%] lg:-translate-y-[5%]">
                    <svg
                      width="24"
                      height="24"
                      viewBox="0 0 32 33"
                      fill="none"
                      xmlns="http://www.w3.org/2000/svg"
                      className="absolute top-1/2 left-1/2 h-8 w-8"
                    >
                      <path
                        d="M8.58243 2.64649C9.68243 2.23399 11.8595 2.48608 12.4324 3.72358C13.0053 4.96108 13.3491 6.56524 13.372 6.17566C13.3282 4.99155 13.4282 3.8065 13.6699 2.64649C13.9246 1.90357 14.5083 1.31996 15.2512 1.06524C15.9325 0.849761 16.6559 0.802581 17.3595 0.927743C18.0709 1.07418 18.7009 1.4833 19.1241 2.07358C19.6602 3.40992 19.9625 4.82851 20.0178 6.26733C20.0748 5.03958 20.2827 3.8235 20.6366 2.64649C21.0195 2.10692 21.5788 1.71789 22.2178 1.54649C22.9755 1.40797 23.7519 1.40797 24.5095 1.54649C25.1314 1.75288 25.6753 2.14475 26.0678 2.66941C26.5546 3.88434 26.8484 5.16789 26.9387 6.47358C26.9387 6.79441 27.0991 5.57983 27.6033 4.77774C28.0083 3.57537 29.3113 2.92898 30.5137 3.33399C31.716 3.739 32.3624 5.04204 31.9574 6.24441C31.9574 7.73399 31.9574 7.66524 31.9574 8.67358C31.9574 9.68191 31.9574 10.5757 31.9574 11.4236C31.8749 12.7647 31.691 14.0977 31.4074 15.4111C31.0097 16.5737 30.4545 17.6763 29.7574 18.6882C28.645 19.9258 27.7256 21.3242 27.0303 22.8361C26.8607 23.5878 26.7838 24.3574 26.8012 25.1277C26.7989 25.8396 26.8914 26.5486 27.0762 27.2361C26.1393 27.3362 25.1943 27.3362 24.2574 27.2361C23.3637 27.0986 22.2637 25.3111 21.9658 24.7611C21.8184 24.4658 21.5167 24.2792 21.1866 24.2792C20.8565 24.2792 20.5548 24.4658 20.4074 24.7611C19.9033 25.6319 18.7803 27.2132 18.1158 27.3048C16.5803 27.4882 13.3949 27.3048 10.9199 27.3048C10.9199 27.3048 11.3553 25.0132 10.3928 24.1882C9.43034 23.3632 8.49076 22.4007 7.78034 21.759L5.87826 19.6507C4.53693 18.4055 3.55538 16.8224 3.03659 15.0673C2.55534 12.9132 2.60117 11.8819 3.03659 11.0111C3.48069 10.292 4.17416 9.76167 4.98451 9.52149C5.65773 9.39937 6.35076 9.44662 7.00117 9.65899C7.45095 9.84729 7.83967 10.1567 8.12409 10.5527C8.65118 11.2632 8.83451 11.6069 8.60534 10.8277C8.37617 10.0486 7.87201 9.47566 7.61993 8.53608C7.12917 7.42645 6.83453 6.24013 6.74909 5.02983C6.84301 3.94395 7.60118 3.03049 8.65118 2.73816"
                        fill="white"
                      />
                      <path
                        fillRule="evenodd"
                        clipRule="evenodd"
                        d="M8.58243 2.64649C9.68243 2.23399 11.8595 2.48608 12.4324 3.72358C13.0053 4.96108 13.3491 6.56524 13.372 6.17566C13.3282 4.99155 13.4282 3.8065 13.6699 2.64649C13.9246 1.90357 14.5083 1.31996 15.2512 1.06524C15.9325 0.849761 16.6559 0.802581 17.3595 0.927743C18.0709 1.07418 18.7009 1.4833 19.1241 2.07358C19.6602 3.40992 19.9625 4.82851 20.0178 6.26733C20.0748 5.03958 20.2827 3.8235 20.6366 2.64649C21.0195 2.10692 21.5788 1.71789 22.2178 1.54649C22.9755 1.40797 23.7519 1.40797 24.5095 1.54649C25.1314 1.75288 25.6753 2.14475 26.0678 2.66941C26.5546 3.88434 26.8484 5.16789 26.9387 6.47358C26.9387 6.79441 27.0991 5.57983 27.6033 4.77774C28.0083 3.57537 29.3113 2.92898 30.5137 3.33399C31.716 3.739 32.3624 5.04204 31.9574 6.24441C31.9574 7.73399 31.9574 7.66524 31.9574 8.67358C31.9574 9.68191 31.9574 10.5757 31.9574 11.4236C31.8749 12.7647 31.691 14.0977 31.4074 15.4111C31.0097 16.5737 30.4545 17.6763 29.7574 18.6882C28.645 19.9258 27.7256 21.3242 27.0303 22.8361C26.8607 23.5878 26.7838 24.3574 26.8012 25.1277C26.7989 25.8396 26.8914 26.5486 27.0762 27.2361C26.1393 27.3362 25.1943 27.3362 24.2574 27.2361C23.3637 27.0986 22.2637 25.3111 21.9658 24.7611C21.8184 24.4658 21.5167 24.2792 21.1866 24.2792C20.8565 24.2792 20.5548 24.4658 20.4074 24.7611C19.9033 25.6319 18.7803 27.2132 18.1158 27.3048C16.5803 27.4882 13.3949 27.3048 10.9199 27.3048C10.9199 27.3048 11.3553 25.0132 10.3928 24.1882C9.43034 23.3632 8.49076 22.4007 7.78034 21.759L5.87826 19.6507C4.53693 18.4055 3.55538 16.8224 3.03659 15.0673C2.55534 12.9132 2.60117 11.8819 3.03659 11.0111C3.48069 10.292 4.17416 9.76167 4.98451 9.52149C5.65773 9.39937 6.35076 9.44662 7.00117 9.65899C7.45095 9.84729 7.83967 10.1567 8.12409 10.5527C8.65117 11.2632 8.83451 11.6069 8.60534 10.8277C8.37618 10.0486 7.87201 9.47566 7.61992 8.53608C7.12917 7.42645 6.83453 6.24013 6.74909 5.02983C6.79595 3.92807 7.52955 2.97439 8.58243 2.64649Z"
                        stroke="black"
                        strokeWidth="1.71875"
                        strokeLinejoin="round"
                      />
                      <path
                        d="M26.3428 20.2369V12.3266C26.3428 11.8531 25.958 11.4692 25.4834 11.4692C25.0088 11.4692 24.624 11.8531 24.624 12.3266V20.2369C24.624 20.7104 25.0088 21.0942 25.4834 21.0942C25.958 21.0942 26.3428 20.7104 26.3428 20.2369Z"
                        fill="black"
                      />
                      <path
                        d="M21.8053 20.234L21.7595 12.3196C21.7568 11.8472 21.3698 11.4665 20.8952 11.4693C20.4206 11.472 20.0381 11.8571 20.0408 12.3295L20.0866 20.2439C20.0894 20.7162 20.4763 21.0969 20.9509 21.0942C21.4255 21.0915 21.8081 20.7064 21.8053 20.234Z"
                        fill="black"
                      />
                      <path
                        d="M15.4575 12.3399L15.5034 20.2337C15.5061 20.7118 15.8931 21.097 16.3678 21.0942C16.8424 21.0914 17.2249 20.7016 17.2221 20.2236L17.1763 12.3297C17.1735 11.8517 16.7865 11.4665 16.3119 11.4693C15.8373 11.472 15.4548 11.8618 15.4575 12.3399Z"
                        fill="black"
                      />
                    </svg>
                    <Icon
                      icon="mdi:file-outline"
                      className="text-fg-muted text-xl"
                    />
                    <div className="flex flex-col">
                      <p className="text-fg-muted text-xs">
                        Meeting.12.03.26.wav
                      </p>
                      <p className="text-fg-subtle text-xs">14:30:25</p>
                    </div>
                  </div>
                </div>
              </div>
              <div className="flex min-h-0 flex-col justify-start gap-2 md:max-h-[200px]">
                <h4 className="text-color mb-4 font-mono text-base font-medium md:text-xl">
                  Upload existing recordings
                </h4>
                <p className="text-color text-base">
                  Drop in audio files or transcripts to turn them into
                  searchable notes.
                </p>
              </div>
            </div>

            {/* languages */}
            <div className="border-brand flex w-[85%] shrink-0 snap-start flex-col gap-2 md:w-auto md:shrink">
              <div className="flex h-32 items-center justify-start select-none md:h-24 lg:h-32">
                <HelloBubble />
              </div>
              <div className="flex min-h-0 flex-col justify-start gap-2 md:max-h-[200px]">
                <h4 className="text-color mb-4 font-mono text-base font-medium md:text-xl">
                  40+ <br /> languages
                </h4>
                <p className="text-color text-base">
                  Char uses best-in-class transcription models and updates them
                  continuously. Speak in the language you think in.
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function ChatBubbleQuestion({ text }: { text: string }) {
  return (
    <div className="flex w-full justify-end">
      <div className="border-color-brand w-2/3 rounded-t-2xl rounded-bl-2xl border bg-blue-50 px-4 py-3">
        <p className="text-color text-sm">{text}</p>
      </div>
    </div>
  );
}

const helloWords = [
  { text: "Hello", lang: "EN" },
  { text: "Hola", lang: "ES" },
  { text: "Bonjour", lang: "FR" },
  { text: "Hallo", lang: "DE" },
  { text: "こんにちは", lang: "JP" },
  { text: "안녕하세요", lang: "KR" },
  { text: "你好", lang: "ZH" },
  { text: "Olá", lang: "PT" },
  { text: "Ciao", lang: "IT" },
  { text: "Привет", lang: "RU" },
  { text: "مرحبا", lang: "AR" },
  { text: "नमस्ते", lang: "HI" },
];

function HelloBubble() {
  const [index, setIndex] = useState(0);
  const { ref, isInView } = useHasEnteredView<HTMLDivElement>(0.4);

  useEffect(() => {
    if (!isInView) {
      return;
    }

    const id = setInterval(() => {
      setIndex((i) => (i + 1) % helloWords.length);
    }, 2000);
    return () => clearInterval(id);
  }, [isInView]);

  return (
    <div ref={ref} className="relative h-[44px] md:h-[34px] lg:h-[44px]">
      <AnimatePresence mode="popLayout">
        <motion.div
          key={index}
          className="flex h-full items-center rounded-full rounded-bl-sm bg-blue-500 px-6 md:px-4 lg:px-6"
          initial={{ y: 20, opacity: 0, filter: "blur(4px)" }}
          animate={{ y: 0, opacity: 1, filter: "blur(0px)" }}
          exit={{ y: -20, opacity: 0, filter: "blur(4px)" }}
          transition={{ duration: 0.4, ease: [0.4, 0, 0.2, 1] }}
        >
          <span className="block text-2xl font-medium whitespace-nowrap text-white md:text-lg lg:text-2xl">
            {helloWords[index].text}
          </span>
        </motion.div>
      </AnimatePresence>
    </div>
  );
}

function ChatBubbleResponse({
  text,
  withCheck,
}: {
  text: string;
  withCheck?: boolean;
}) {
  return (
    <div className="border-color-brand w-2/3 rounded-t-xl rounded-br-xl border bg-gradient-to-b from-white to-stone-100 px-4 py-3">
      <p className="text-fg-muted mb-1 text-sm">Char</p>
      {withCheck ? (
        <div className="flex items-center gap-2 text-sm">
          <Icon icon="mdi:check-circle" className="text-sm text-green-500" />
          <span className="text-color">{text}</span>
        </div>
      ) : (
        <p className="text-color text-sm">{text}</p>
      )}
    </div>
  );
}

function ChatPanel({ children }: { children: React.ReactNode }) {
  return (
    <div className="border-color-brand surface flex w-full flex-col overflow-hidden rounded-xl border shadow-xl">
      <div className="border-color-brand flex h-9 shrink-0 items-center border-b px-3">
        <div className="flex items-center gap-2">
          <Icon
            icon="mdi:message-text-outline"
            className="text-sm text-neutral-400"
          />
          <span className="text-xs font-medium text-neutral-700">Chat</span>
        </div>
      </div>
      <div className="flex min-h-[300px] flex-col justify-end p-3">
        {children}
      </div>
      <div className="border-color-brand shrink-0 border-t px-3 py-2.5">
        <div className="flex items-center justify-between">
          <span className="text-sm text-neutral-400">
            Ask about your notes...
          </span>
          <div className="border-color-brand inline-flex h-7 items-center rounded-lg border px-2.5 text-xs font-medium text-neutral-300">
            <span>Send</span>
          </div>
        </div>
      </div>
    </div>
  );
}

const cyclingPairs = [
  {
    q: "What did Sarah say about the timeline?",
    a: "Sarah mentioned the mobile redesign needs 2 sprints, with the first focused on core navigation improvements.",
  },
  {
    q: "Any action items from last week's sync?",
    a: "Ben to finish auth module by Friday. Sarah to share updated API specs. Victor to review the dashboard mockups.",
  },
  {
    q: "What decisions were made in Q1 planning?",
    a: "Team agreed to prioritize mobile UI over the new dashboard. API adjustments will be scoped in the next sprint.",
  },
];

function CyclingChatGraphic() {
  const [index, setIndex] = useState(0);
  const [phase, setPhase] = useState<"question" | "answer" | "exit">(
    "question",
  );
  const { ref, isInView } = useHasEnteredView<HTMLDivElement>(0.4);

  useEffect(() => {
    if (!isInView) {
      return;
    }

    const timers: ReturnType<typeof setTimeout>[] = [];

    function runCycle() {
      setPhase("question");

      timers.push(setTimeout(() => setPhase("answer"), 800));
      timers.push(setTimeout(() => setPhase("exit"), 3500));
      timers.push(
        setTimeout(() => {
          setIndex((i) => (i + 1) % cyclingPairs.length);
          setPhase("question");
        }, 4000),
      );
    }

    runCycle();
    const id = setInterval(runCycle, 4000);

    return () => {
      clearInterval(id);
      timers.forEach(clearTimeout);
    };
  }, [isInView]);

  const pair = cyclingPairs[index];

  return (
    <div ref={ref} className="flex w-full flex-col">
      <AnimatePresence mode="wait">
        {phase !== "exit" && (
          <motion.div
            key={index}
            initial={{ opacity: 1 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0, y: -20 }}
            transition={{ duration: 0.35, ease: "easeInOut" }}
            className="flex flex-col"
          >
            <motion.div
              layout
              initial={{ opacity: 0, y: 12 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{
                duration: 0.3,
                ease: "easeOut",
                layout: { duration: 0.4, ease: [0.4, 0, 0.2, 1] },
              }}
            >
              <ChatBubbleQuestion text={pair.q} />
            </motion.div>
            <AnimatePresence>
              {phase === "answer" && (
                <motion.div
                  initial={{ opacity: 0, height: 0, marginTop: 0 }}
                  animate={{ opacity: 1, height: "auto", marginTop: 12 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.4, ease: [0.4, 0, 0.2, 1] }}
                  style={{ overflow: "hidden" }}
                >
                  <ChatBubbleResponse text={pair.a} />
                </motion.div>
              )}
            </AnimatePresence>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function MeetingBar({ animated = true }: { animated?: boolean }) {
  return (
    <div className="flex w-full items-center justify-between rounded-full bg-stone-700 p-2 pl-6">
      <div className="flex items-center gap-3">
        <div className="relative flex size-3">
          <span
            className={cn([
              "absolute inline-flex size-full rounded-full bg-red-400 opacity-75",
              animated && "animate-ping",
            ])}
          />
          <span className="relative inline-flex size-3 rounded-full bg-red-500" />
        </div>
        <p className="text-sm text-white">Weekly Team Sync</p>
        <span className="text-xs text-white/50">42:17</span>
      </div>
      <div className="flex items-center gap-2">
        <div className="flex items-center justify-center rounded-full px-2">
          <Icon icon="mdi:dots-horizontal" className="text-xl text-white/60" />
        </div>
        <div className="flex items-center justify-center rounded-full bg-red-600 px-3 py-2">
          <Icon icon="mdi:phone-hangup" className="text-xl text-white" />
        </div>
      </div>
    </div>
  );
}

function LiveChatMessages() {
  const [step, setStep] = useState(0);
  const { ref, isInView } = useHasEnteredView<HTMLDivElement>(0.4);

  useEffect(() => {
    if (!isInView) {
      return;
    }

    setStep(0);
    const t1 = setTimeout(() => setStep(1), 600);
    const t2 = setTimeout(() => setStep(2), 1200);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
    };
  }, [isInView]);

  const ease = [0.4, 0, 0.2, 1] as const;

  return (
    <div ref={ref} className="flex flex-col">
      <motion.div
        initial={{ opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3, ease: "easeOut" }}
      >
        <ChatBubbleQuestion text="What's the timeline for the mobile UI?" />
      </motion.div>
      <AnimatePresence>
        {step >= 1 && (
          <motion.div
            initial={{ opacity: 0, height: 0, marginTop: 0 }}
            animate={{ opacity: 1, height: "auto", marginTop: 8 }}
            transition={{ duration: 0.4, ease }}
            style={{ overflow: "hidden" }}
          >
            <TranscriptToolCall loopKey={0} static />
          </motion.div>
        )}
      </AnimatePresence>
      <AnimatePresence>
        {step >= 2 && (
          <motion.div
            initial={{ opacity: 0, height: 0, marginTop: 0 }}
            animate={{ opacity: 1, height: "auto", marginTop: 8 }}
            transition={{ duration: 0.4, ease }}
            style={{ overflow: "hidden" }}
          >
            <ChatBubbleResponse text="Ben committed to auth module this week. Sarah estimates 2 sprints for full API." />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function BeforeMeetingMessages() {
  const [step, setStep] = useState(0);
  const { ref, isInView } = useHasEnteredView<HTMLDivElement>(0.4);

  useEffect(() => {
    if (!isInView) {
      return;
    }

    setStep(0);
    const t1 = setTimeout(() => setStep(1), 600);
    const t2 = setTimeout(() => setStep(2), 2400);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
    };
  }, [isInView]);

  const ease = [0.4, 0, 0.2, 1] as const;

  return (
    <div ref={ref} className="flex flex-col">
      <motion.div
        initial={{ opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3, ease: "easeOut" }}
      >
        <ChatBubbleQuestion text="What was my last conversation with Sarah about?" />
      </motion.div>
      <AnimatePresence>
        {step >= 1 && (
          <motion.div
            initial={{ opacity: 0, height: 0, marginTop: 0 }}
            animate={{ opacity: 1, height: "auto", marginTop: 8 }}
            transition={{ duration: 0.4, ease }}
            style={{ overflow: "hidden" }}
          >
            <ContactSearchToolCall loopKey={0} static />
          </motion.div>
        )}
      </AnimatePresence>
      <AnimatePresence>
        {step >= 2 && (
          <motion.div
            initial={{ opacity: 0, height: 0, marginTop: 0 }}
            animate={{ opacity: 1, height: "auto", marginTop: 8 }}
            transition={{ duration: 0.4, ease }}
            style={{ overflow: "hidden" }}
          >
            <ChatBubbleResponse text="Your last meeting with Sarah was about Q2 roadmap priorities. She proposed 2 sprints for mobile redesign and you agreed to share updated specs by Friday." />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

export function AISection() {
  const [activeBlock, setActiveBlock] = useState<0 | 1 | 2>(0);
  const { ref, isInView } = useHasEnteredView<HTMLElement>(0.2);

  return (
    <section ref={ref} id="ai" className="px-4 py-16">
      <div className="items-left flex flex-col gap-4 pb-12 text-left">
        <h2 className="text-fg font-mono text-2xl tracking-wide md:text-4xl">
          Get more from every note with AI
        </h2>
        <p className="text-fg-muted">
          Ask questions, execute tasks, and grow your knowledge base—all from
          your meeting notes.
        </p>
      </div>

      <div className="surface-subtle border-color-brand flex flex-col overflow-hidden rounded-xl border md:flex-row">
        <div className="flex flex-col gap-12 px-2 pt-8 pb-12 md:w-1/2 md:pt-12 md:pr-8 md:pb-32 md:pl-4">
          {/* Block 0: Before meeting */}
          <div
            className={cn([
              "flex cursor-pointer flex-col gap-2 border-l-2 pl-2 transition-all duration-200 md:pl-4",
              activeBlock === 0
                ? "md:border-l-stone-800"
                : "opacity-50 hover:opacity-75 md:border-l-transparent",
            ])}
            onMouseEnter={() => setActiveBlock(0)}
          >
            <p className="text-fg-muted font-mono text-xs tracking-widest uppercase">
              Before meeting
            </p>
            <p className="text-color font-regular text-lg leading-relaxed md:text-2xl">
              Get a quick brief before the call
            </p>
            <p className="text-color-muted text-base leading-relaxed">
              Get the relevant info about people, goals and previous meetings.
              Char links contacts and conversations, search through them and get
              a whole picture.
            </p>
          </div>

          {/* Mobile image for block 0 */}
          <div className="bg-dotted-dark flex min-h-[280px] flex-col justify-center gap-3 p-2 md:hidden">
            <ChatPanel>
              <BeforeMeetingMessages />
            </ChatPanel>
          </div>

          {/* Block 1: During meeting */}
          <div
            className={cn([
              "flex cursor-pointer flex-col gap-2 border-l-2 pl-4 transition-all duration-200",
              activeBlock === 1
                ? "md:border-l-stone-800"
                : "md:border-l-transparent md:opacity-50 md:hover:opacity-75",
            ])}
            onMouseEnter={() => setActiveBlock(1)}
          >
            <p className="text-fg-muted font-mono text-xs tracking-widest uppercase">
              During meeting
            </p>
            <p className="text-color font-regular text-lg leading-relaxed md:text-2xl">
              Chat during live meetings
            </p>
            <p className="text-color-muted text-base leading-relaxed">
              Get instant answers from the current transcript and past meeting
              context without breaking your flow.
            </p>
          </div>

          {/* Mobile image for block 1 */}
          <div className="bg-dotted-dark flex min-h-[280px] flex-col justify-center gap-3 p-8 md:hidden">
            <MeetingBar animated={isInView} />
            <ChatPanel>
              <LiveChatMessages />
            </ChatPanel>
          </div>

          {/* Block 2: After meeting */}
          <div
            className={cn([
              "flex cursor-pointer flex-col gap-2 border-l-2 pl-4 transition-all duration-200",
              activeBlock === 2
                ? "border-l-stone-800"
                : "border-l-transparent opacity-50 hover:opacity-75",
            ])}
            onMouseEnter={() => setActiveBlock(2)}
          >
            <p className="text-fg-muted font-mono text-xs tracking-widest uppercase">
              After meeting
            </p>
            <p className="text-color font-regular text-lg leading-relaxed md:text-2xl">
              Chat with your notes
            </p>
            <p className="text-color-muted text-base leading-relaxed">
              Query your entire conversation history. Find decisions, action
              items, or topics discussed in previous meetings in natural
              language.
            </p>
          </div>

          {/* Mobile image for block 2 */}
          <div className="bg-dotted-dark flex min-h-[320px] items-end justify-center p-8 md:hidden">
            <ChatPanel>
              <CyclingChatGraphic />
            </ChatPanel>
          </div>
        </div>

        {/* Desktop right panel */}
        <div className="bg-dotted-dark hidden flex-col justify-end gap-2 p-8 md:flex md:w-1/2">
          <AnimatePresence mode="wait">
            {activeBlock === 1 && (
              <motion.div
                key="meeting-bar"
                initial={{ opacity: 0, y: -8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -8 }}
                transition={{ duration: 0.3 }}
              >
                <MeetingBar animated={isInView} />
              </motion.div>
            )}
          </AnimatePresence>
          <ChatPanel>
            <AnimatePresence mode="wait">
              {activeBlock === 0 && (
                <motion.div
                  key="before"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.3 }}
                  className="w-full"
                >
                  <BeforeMeetingMessages />
                </motion.div>
              )}
              {activeBlock === 1 && (
                <motion.div
                  key="live"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.3 }}
                  className="w-full"
                >
                  <LiveChatMessages />
                </motion.div>
              )}
              {activeBlock === 2 && (
                <motion.div
                  key="cycling"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.3 }}
                  className="w-full"
                >
                  <CyclingChatGraphic />
                </motion.div>
              )}
            </AnimatePresence>
          </ChatPanel>
        </div>
      </div>
    </section>
  );
}

export function GrowsWithYouSection() {
  return (
    <section id="grows-with-you" className="px-4 pt-8 pb-16">
      <div className="surface border-color-brand mx-auto rounded-xl border">
        <div className="items-left flex flex-col gap-2 px-8 pt-16 pb-8 text-left">
          <h2 className="text-color font-mono text-2xl tracking-wide md:text-4xl">
            Char grows with you
          </h2>
          <p className="text-md text-color-muted max-w-2xl pb-4">
            Add people from meetings in contacts, grow knowledge about your
            chats and context of previous meetings
          </p>
          <Link
            to="/product/mini-apps/"
            className="text-md text-color-muted hover:text-color flex items-center gap-1 underline"
          >
            Explore all features
            <Icon icon="mdi:arrow-top-right" className="text-sm" />
          </Link>
        </div>

        <div className="border-color-brand grid border-t md:grid-cols-2">
          <div className="bg-lined-notebook border-color-brand flex flex-col border-b md:border-r md:border-b-0">
            <div className="flex h-[240px] items-start px-8 pt-8">
              <div className="surface border-color-brand w-full rounded-xl border p-4 md:max-w-4/5">
                <div className="mb-3 flex items-center gap-3">
                  <div className="flex size-10 items-center justify-center rounded-full bg-blue-100 text-sm font-bold text-blue-500">
                    S
                  </div>
                  <div>
                    <p className="text-color text-sm font-medium">Sarah Chen</p>
                    <p className="text-color-muted text-xs">
                      Product Lead · Acme Inc
                    </p>
                  </div>
                </div>
                <div className="text-color-muted mb-2 text-xs">
                  sarah@acme.com · +1 (415) 555-0123
                </div>
                <div className="border-color-brand bg-surface-subtle rounded border p-3">
                  <p className="text-color-muted mb-1 text-xs font-medium">
                    Last conversation
                  </p>
                  <p className="text-color-muted text-xs">
                    Discussed Q2 roadmap priorities and timeline for the mobile
                    redesign. Agreed to share updated specs by Friday.
                  </p>
                </div>
              </div>
            </div>
            <div className="px-8 pt-8 pb-8">
              <h3 className="text-color mb-3 font-mono text-2xl leading-[1.3]">
                Have your contacts in one place
              </h3>
              <p className="text-color-muted mb-4 text-base leading-relaxed md:max-w-2/3">
                Import contacts and watch them come alive with context once you
                actually meet.
              </p>
              <ul className="flex flex-col gap-3">
                <li className="flex items-start gap-3">
                  <span className="text-md text-color-muted">
                    All your chats linked
                  </span>
                </li>
                <li className="flex items-start gap-3">
                  <span className="text-md text-color-muted">
                    Generate summaries from meetings
                  </span>
                </li>
              </ul>
            </div>
          </div>
          <div className="bg-grid flex flex-col">
            <div className="flex h-[240px] items-center px-8 pt-8">
              <div className="surface-subtle border-color-brand flex w-full items-center justify-between gap-4 rounded-2xl border p-4 md:max-w-4/5">
                <div className="flex items-center gap-3">
                  <Icon
                    icon="mdi:calendar"
                    className="text-color-muted text-xl"
                  />
                  <div>
                    <p className="text-color text-sm font-medium">
                      Weekly Team Sync
                    </p>
                    <p className="text-color-muted text-xs">
                      Starting in 2 min
                    </p>
                  </div>
                </div>
                <button className="bg-brand-color shrink-0 rounded-full bg-stone-700 px-4 py-2 text-xs font-medium text-white shadow-md transition-shadow duration-200 hover:shadow-lg">
                  Start listening
                </button>
              </div>
            </div>
            <div className="px-8 pt-8 pb-8">
              <h3 className="text-color mb-3 font-mono text-2xl">
                Work with your calendar
              </h3>
              <p className="text-color-muted mb-4 text-base leading-relaxed">
                Connect your calendar for intelligent meeting preparation and
                automatic note organization.
              </p>
              <ul className="flex flex-col gap-3">
                <li className="flex items-start gap-3">
                  <span className="text-md text-color-muted">
                    Automatic meeting linking
                  </span>
                </li>
                <li className="flex items-start gap-3">
                  <span className="text-md text-color-muted">
                    Pre-meeting context and preparation
                  </span>
                </li>
                <li className="flex items-start gap-3">
                  <span className="text-md text-color-muted">
                    Timeline view with notes
                  </span>
                </li>
              </ul>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

export function MainFeaturesSection({
  featuresScrollRef,
  selectedFeature,
  setSelectedFeature,
  scrollToFeature,
}: {
  featuresScrollRef: React.RefObject<HTMLDivElement | null>;
  selectedFeature: number;
  setSelectedFeature: (index: number) => void;
  scrollToFeature: (index: number) => void;
}) {
  const [progress, setProgress] = useState(0);
  const progressRef = useRef(0);
  const { ref, isInView } = useHasEnteredView<HTMLElement>(0.2);

  const handleFeatureIndexChange = useCallback(
    (nextIndex: number) => {
      setSelectedFeature(nextIndex);
      setProgress(0);
      progressRef.current = 0;
    },
    [setSelectedFeature],
  );

  useEffect(() => {
    if (!isInView) {
      return;
    }

    const startTime =
      Date.now() - (progressRef.current / 100) * FEATURES_AUTO_ADVANCE_DURATION;
    let animationId: number;

    const animate = () => {
      const elapsed = Date.now() - startTime;
      const newProgress = Math.min(
        (elapsed / FEATURES_AUTO_ADVANCE_DURATION) * 100,
        100,
      );
      setProgress(newProgress);
      progressRef.current = newProgress;

      if (newProgress >= 100) {
        const currentActiveIndex =
          activeFeatureIndices.indexOf(selectedFeature);
        const nextActiveIndex =
          (currentActiveIndex + 1) % activeFeatureIndices.length;
        const nextIndex = activeFeatureIndices[nextActiveIndex];
        setSelectedFeature(nextIndex);
        setProgress(0);
        progressRef.current = 0;
        if (featuresScrollRef.current) {
          const container = featuresScrollRef.current;
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
  }, [featuresScrollRef, isInView, selectedFeature, setSelectedFeature]);

  const handleScrollToFeature = (index: number) => {
    scrollToFeature(index);
    setProgress(0);
    progressRef.current = 0;
  };

  return (
    <section ref={ref}>
      <div className="px-4 py-16 text-left">
        <div className="mx-auto mb-6 flex size-28 items-center justify-center rounded-4xl border border-neutral-100 bg-transparent shadow-xl">
          <Image
            src="/api/images/hyprnote/icon.png"
            alt="Char"
            width={96}
            height={96}
            className="size-24 rounded-3xl border border-neutral-100"
          />
        </div>
        <h2 className="text-color mb-4 font-mono text-2xl tracking-wide md:text-4xl">
          Works like charm
        </h2>
        <p className="text-fg-muted mx-auto max-w-lg">
          {
            "Super simple and easy to use with its clean interface. And it's getting better with every update — every single week."
          }
        </p>
      </div>
      <FeaturesMobileCarousel
        featuresScrollRef={featuresScrollRef}
        selectedFeature={selectedFeature}
        onIndexChange={handleFeatureIndexChange}
        scrollToFeature={handleScrollToFeature}
        progress={progress}
      />
      <FeaturesDesktopGrid />
    </section>
  );
}

function FeaturesMobileCarousel({
  featuresScrollRef,
  selectedFeature,
  onIndexChange,
  scrollToFeature,
  progress,
}: {
  featuresScrollRef: React.RefObject<HTMLDivElement | null>;
  selectedFeature: number;
  onIndexChange: (index: number) => void;
  scrollToFeature: (index: number) => void;
  progress: number;
}) {
  const isSwiping = useRef(false);

  return (
    <div className="hidden max-[800px]:block">
      <div
        ref={featuresScrollRef}
        className="scrollbar-hide snap-x snap-mandatory overflow-x-auto"
        onTouchStart={() => {
          isSwiping.current = true;
          onIndexChange(selectedFeature);
        }}
        onTouchEnd={() => {
          isSwiping.current = false;
        }}
        onScroll={(e) => {
          const container = e.currentTarget;
          const scrollLeft = container.scrollLeft;
          const itemWidth = container.offsetWidth;
          const index = Math.round(scrollLeft / itemWidth);
          if (index !== selectedFeature) {
            onIndexChange(index);
          }
        }}
      >
        <div className="flex">
          {mainFeatures.map((feature, index) => (
            <div key={index} className="w-full shrink-0 snap-center">
              <div className="flex flex-col overflow-hidden border-y border-neutral-100">
                <Link
                  to={feature.link}
                  className={cn([
                    "relative block aspect-video overflow-hidden border-b border-neutral-100",
                    (feature.image || feature.muxPlaybackId) &&
                      "bg-neutral-100",
                  ])}
                >
                  {feature.muxPlaybackId ? (
                    <MobileFeatureVideo
                      playbackId={feature.muxPlaybackId}
                      alt={`${feature.title} feature`}
                      isActive={selectedFeature === index}
                    />
                  ) : feature.image ? (
                    <Image
                      src={feature.image}
                      alt={`${feature.title} feature`}
                      className="h-full w-full object-contain"
                    />
                  ) : (
                    <img
                      src="/api/images/hyprnote/static.webp"
                      alt={`${feature.title} feature`}
                      className="h-full w-full object-cover"
                    />
                  )}
                </Link>
                <div className="p-6">
                  <div className="mb-2 flex items-center gap-3">
                    <Icon
                      icon={feature.icon}
                      className="text-fg-muted text-2xl"
                    />
                    <h3 className="text-color font-mono text-lg">
                      {feature.title}
                    </h3>
                  </div>
                  <p className="text-fg-muted text-base">
                    {feature.description}
                  </p>
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="flex justify-center gap-2 py-6">
        {mainFeatures.map((_, index) => (
          <button
            key={index}
            onClick={() => scrollToFeature(index)}
            className={cn([
              "h-1 cursor-pointer overflow-hidden rounded-full",
              selectedFeature === index
                ? "w-8 bg-neutral-300"
                : "w-8 bg-neutral-300 hover:bg-neutral-400",
            ])}
            aria-label={`Go to feature ${index + 1}`}
          >
            {selectedFeature === index && (
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

function MobileFeatureVideo({
  playbackId,
  alt,
  isActive,
}: {
  playbackId: string;
  alt: string;
  isActive: boolean;
}) {
  const playerRef = useRef<MuxPlayerRefAttributes>(null);
  const { ref, isInView, hasEnteredView } =
    useHasEnteredView<HTMLDivElement>(0.35);
  const thumbnailUrl = `https://image.mux.com/${playbackId}/thumbnail.jpg?width=1920&height=1080&fit_mode=smartcrop`;
  const shouldLoadPlayer = hasEnteredView || isActive;

  useEffect(() => {
    const player = playerRef.current;
    if (!player) return;

    if (isActive && isInView) {
      player.play()?.catch(() => {
        // Autoplay blocked or player not ready - fail silently
      });
    } else {
      player.pause();
      player.currentTime = 0;
    }
  }, [isActive, isInView]);

  return (
    <div ref={ref} className="relative h-full w-full">
      <img
        src={thumbnailUrl}
        alt={alt}
        className={cn([
          "absolute inset-0 h-full w-full object-contain transition-opacity duration-300",
          isActive ? "opacity-0" : "opacity-100",
        ])}
      />
      {shouldLoadPlayer && (
        <MuxPlayer
          ref={playerRef}
          playbackId={playbackId}
          muted
          loop
          playsInline
          maxResolution="1080p"
          minResolution="720p"
          className={cn([
            "h-full w-full object-contain transition-opacity duration-300",
            isActive ? "opacity-100" : "opacity-0",
          ])}
          style={
            {
              "--controls": "none",
            } as React.CSSProperties & { [key: `--${string}`]: string }
          }
        />
      )}
    </div>
  );
}

function FeatureVideo({
  playbackId,
  alt,
  isHovered,
}: {
  playbackId: string;
  alt: string;
  isHovered: boolean;
}) {
  const playerRef = useRef<MuxPlayerRefAttributes>(null);
  const { ref, isInView, hasEnteredView } =
    useHasEnteredView<HTMLDivElement>(0.35);
  const thumbnailUrl = `https://image.mux.com/${playbackId}/thumbnail.jpg?width=1920&height=1080&fit_mode=smartcrop`;
  const shouldLoadPlayer = hasEnteredView || isHovered;

  useEffect(() => {
    const player = playerRef.current;
    if (!player) return;

    if (isHovered && isInView) {
      player.play();
    } else {
      player.pause();
      player.currentTime = 0;
    }
  }, [isHovered, isInView]);

  return (
    <div ref={ref} className="relative h-full w-full">
      <img
        src={thumbnailUrl}
        alt={alt}
        className={cn([
          "absolute inset-0 h-full w-full object-contain transition-opacity duration-300",
          isHovered ? "opacity-0" : "opacity-100",
        ])}
      />
      {shouldLoadPlayer && (
        <MuxPlayer
          ref={playerRef}
          playbackId={playbackId}
          muted
          loop
          playsInline
          maxResolution="1080p"
          minResolution="720p"
          className={cn([
            "h-full w-full object-contain transition-opacity duration-300",
            isHovered ? "opacity-100" : "opacity-0",
          ])}
          style={
            {
              "--controls": "none",
            } as React.CSSProperties & { [key: `--${string}`]: string }
          }
        />
      )}
    </div>
  );
}

function FeaturesDesktopGrid() {
  const [hoveredFeature, setHoveredFeature] = useState<number | null>(null);

  const gridClasses = [
    "col-span-6 md:col-span-3 border-r border-b",
    "col-span-6 md:col-span-3 border-b",
    "col-span-6 md:col-span-2 border-r",
    "col-span-6 md:col-span-2 border-r",
    "col-span-6 md:col-span-2",
  ];

  return (
    <div className="hidden grid-cols-6 min-[800px]:grid">
      {mainFeatures.map((feature, index) => (
        <div
          key={index}
          className={cn(
            gridClasses[index],
            "flex flex-col overflow-hidden border-neutral-100",
          )}
        >
          <Link
            to={feature.link}
            className={cn([
              "group relative block aspect-video overflow-hidden border-b border-neutral-100",
              (feature.image || feature.muxPlaybackId) && "bg-neutral-100",
            ])}
            onMouseEnter={() => setHoveredFeature(index)}
            onMouseLeave={() => setHoveredFeature(null)}
          >
            {feature.muxPlaybackId ? (
              <FeatureVideo
                playbackId={feature.muxPlaybackId}
                alt={`${feature.title} feature`}
                isHovered={hoveredFeature === index}
              />
            ) : feature.image ? (
              <Image
                src={feature.image}
                alt={`${feature.title} feature`}
                className="h-full w-full object-contain"
              />
            ) : (
              <img
                src="/api/images/hyprnote/static.webp"
                alt={`${feature.title} feature`}
                className="h-full w-full object-cover"
              />
            )}
          </Link>
          <div className="flex-1 p-6">
            <div className="mb-2 flex items-center gap-3">
              <Icon icon={feature.icon} className="text-fg-muted text-2xl" />
              <h3 className="text-color font-mono text-lg">{feature.title}</h3>
            </div>
            <p className="text-fg-muted text-base">{feature.description}</p>
          </div>
        </div>
      ))}
    </div>
  );
}

const templateCategories = [
  {
    icon: "mdi:handshake-outline",
    category: "Sales",
    description: "Close deals with organized discovery and follow-ups",
    templates: ["Sales Discovery Call", "Client Kickoff", "Investor Pitch"],
  },
  {
    icon: "mdi:lightbulb-outline",
    category: "Product",
    description: "Build the right things with clear alignment",
    templates: [
      "Product Roadmap Review",
      "Brainstorming Session",
      "Project Kickoff",
    ],
  },
  {
    icon: "mdi:code-braces",
    category: "Engineering",
    description: "Ship faster with focused technical discussions",
    templates: [
      "Sprint Planning",
      "Sprint Retrospective",
      "Technical Design Review",
    ],
  },
];

export function TemplatesSection() {
  return (
    <section>
      <div className="laptop:px-0 px-4 py-12 text-left">
        <h2 className="text-color mb-4 font-mono text-2xl tracking-wide md:text-4xl">
          A template for every meeting
        </h2>
        <p className="text-fg-muted">
          Char adapts to how you work with customizable templates for any
          meeting type
        </p>
      </div>

      <TemplatesMobileView />
      <TemplatesDesktopView />

      <div className="border-t border-neutral-100 py-8 text-left">
        <Link
          to="/gallery/"
          search={{ type: "template" }}
          className={cn([
            "inline-flex items-center gap-2",
            "text-fg-muted hover:text-color",
            "font-medium transition-colors",
          ])}
        >
          View all templates
          <Icon icon="mdi:arrow-right" className="text-lg" />
        </Link>
      </div>
    </section>
  );
}

function TemplatesMobileView() {
  return (
    <div className="border-t border-neutral-100 md:hidden">
      {templateCategories.map((category, index) => (
        <div
          key={category.category}
          className={cn([
            "p-6",
            index < templateCategories.length - 1 &&
              "border-b border-neutral-100",
          ])}
        >
          <div className="mb-3 flex items-center gap-3">
            <Icon icon={category.icon} className="text-fg-muted text-2xl" />
            <h3 className="text-color font-mono text-lg">
              {category.category}
            </h3>
          </div>
          <p className="text-fg-muted mb-4 text-base">{category.description}</p>
          <div className="text-left">
            {category.templates.map((template, i) => (
              <span
                key={template}
                className="text-fg-subtle font-mono text-[11px]"
              >
                {template}
                {i < category.templates.length - 1 ? ", " : ""}
              </span>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

function TemplatesDesktopView() {
  return (
    <div className="hidden grid-cols-3 border-t border-neutral-100 md:grid">
      {templateCategories.map((category, index) => (
        <div
          key={category.category}
          className={cn([
            "p-6",
            index < templateCategories.length - 1 &&
              "border-r border-neutral-100",
          ])}
        >
          <div className="mb-3 flex items-center gap-3">
            <Icon icon={category.icon} className="text-fg-muted text-2xl" />
            <h3 className="text-color font-mono text-lg">
              {category.category}
            </h3>
          </div>
          <p className="text-fg-muted mb-4 text-base">{category.description}</p>
          <div className="text-left">
            {category.templates.map((template, i) => (
              <span
                key={template}
                className="text-fg-subtle font-mono text-[11px]"
              >
                {template}
                {i < category.templates.length - 1 ? ", " : ""}
              </span>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

const solutionColors: Record<
  string,
  { accent: string; bg: string; border: string }
> = {
  developers: {
    accent: "oklch(0.55 0.2245 292.58)",
    bg: "#f5f3ff",
    border: "#ddd6fe",
  },
  enterprise: {
    accent: "#374151",
    bg: "#f9fafb",
    border: "#d1d5db",
  },
  research: {
    accent: "oklch(0.5471 0.1899 264.38)",
    bg: "#eff6ff",
    border: "#bfdbfe",
  },
};

const solutionScenarios = [
  {
    id: "developers",
    label: "Developers",
    headline: "The only meeting AI you can fork, fix and make your own",
    description:
      "Build React extensions, automate with shell hooks, bring your own keys. Self-host or run local. No proprietary modules, just open source code you can inspect and modify.",
    pills: [
      "Bring Your Own Key",
      "Automation Hooks",
      "Fully Extensible",
      "CLI Access",
      "REST API",
    ],
    link: "/solution/engineering",
  },
  {
    id: "enterprise",
    label: "Enterprise",
    headline: "Meeting AI configured for your organization",
    description:
      "Other AI note-takers ask you to trust their infrastructure, their models, and their policies. We built one where you control all three.",
    pills: [
      "Self-Hosted Deployment",
      "Zero-Knowledge Security",
      "Compliance Ready",
      "Access Control",
      "Open Source",
    ],
    link: "/enterprise",
  },
  {
    id: "research",
    label: "Research",
    headline: "Discover faster with AI-powered meeting notes",
    description:
      "Focus on asking questions and observing while Char captures every detail, identifies themes, and helps you analyze research conversations.",
    pills: [
      "Interview Recording",
      "Theme Identification",
      "Quote Extraction",
      "Research Synthesis",
      "Participant Privacy",
    ],
    link: "/solution/research",
  },
];

function SolutionsTabbar() {
  const [activeId, setActiveId] = useState(solutionScenarios[0].id);
  const active =
    solutionScenarios.find((s) => s.id === activeId) ?? solutionScenarios[0];
  const activeColor = solutionColors[active.id];

  return (
    <section id="solutions" className="pb-24 pl-4 md:px-4">
      <div className="mb-8 flex flex-col gap-2 pt-16">
        <h2 className="text-color font-mono text-2xl tracking-wide md:text-4xl">
          Build for every conversation
        </h2>
      </div>

      {/* Folder tabs */}
      <div className="flex h-16 items-end overflow-x-auto [scrollbar-width:none]">
        {solutionScenarios.map((scenario, i) => {
          const isActive = scenario.id === activeId;
          const activeIndex = solutionScenarios.findIndex(
            (s) => s.id === activeId,
          );
          const c = solutionColors[scenario.id];
          const distance = Math.abs(i - activeIndex);
          const z = isActive
            ? solutionScenarios.length + 1
            : solutionScenarios.length - distance;
          const r = 14;
          const isFirst = i === 0;
          const maskCenter = `radial-gradient(${r}px at ${r}px 0, #0000 98%, #000 101%) calc(-1 * ${r}px) 100% / 100% ${r}px repeat-x, conic-gradient(#000 0 0) padding-box`;
          const maskRight = `radial-gradient(${r}px at 100% 0, #0000 98%, #000 101%) 100% 100% / ${r}px ${r}px no-repeat, conic-gradient(#000 0 0) padding-box`;

          return (
            <button
              key={scenario.id}
              onClick={() => setActiveId(scenario.id)}
              style={{
                zIndex: z,
                position: "relative",
                marginRight:
                  i < solutionScenarios.length - 1 ? `-${r + 6}px` : "0",
                marginBottom: 0,
                ...(isFirst
                  ? {
                      borderRight: `${r}px solid transparent`,
                      borderRadius: `${r}px ${2 * r}px 0 0 / ${r}px`,
                      mask: maskRight,
                      WebkitMask: maskRight,
                    }
                  : {
                      borderInline: `${r}px solid transparent`,
                      borderRadius: `${2 * r}px ${2 * r}px 0 0 / ${r}px`,
                      mask: maskCenter,
                      WebkitMask: maskCenter,
                    }),
                background: `${isActive ? c.accent : c.bg} border-box`,
                color: isActive ? "#ffffff" : c.accent,
                transition:
                  "padding-bottom 0.15s ease, margin-bottom 0.15s ease",
              }}
              className={cn([
                "min-w-0 flex-1 cursor-pointer px-3 py-3 text-sm font-medium transition-colors hover:pb-6 md:flex-initial md:shrink-0 md:px-4 md:text-lg",
                isActive ? "pt-2 pb-4" : "",
              ])}
            >
              {scenario.label}
            </button>
          );
        })}
      </div>

      {/* Content block */}
      <div className="relative">
        {/* First tab extension behind body */}
        <div
          className="absolute top-0 left-0"
          style={{
            width: 120,
            height: 24,
            zIndex: -1,
            backgroundColor:
              activeId === solutionScenarios[0].id
                ? solutionColors[solutionScenarios[0].id].accent
                : solutionColors[solutionScenarios[0].id].bg,
            borderRadius: "0 0 12px 12px",
          }}
        />
        <div
          style={{
            backgroundColor: activeColor.accent,
          }}
          className="relative z-0 overflow-hidden rounded-l-xl md:rounded-xl"
        >
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={activeId}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.12 }}
              className="flex h-[480px] flex-col gap-4 px-8 py-16"
            >
              <h3 className="mb-2 max-w-2xl font-mono text-2xl leading-snug text-white md:text-3xl">
                {active.headline}
              </h3>
              <p className="max-w-2xl text-base leading-relaxed text-white">
                {active.description}
              </p>
              <div className="mt-4 flex flex-wrap gap-2">
                {active.pills.map((pill) => (
                  <span
                    key={pill}
                    className="rounded-full border bg-white px-4 py-2 text-base font-medium"
                    style={{ color: activeColor.accent }}
                  >
                    {pill}
                  </span>
                ))}
              </div>
              <a
                href={active.link}
                className="mt-4 flex items-center gap-1 text-sm text-white underline underline-offset-2 hover:text-white/80"
              >
                Learn more
                <Icon icon="mdi:arrow-top-right" className="text-sm" />
              </a>
            </motion.div>
          </AnimatePresence>
        </div>
      </div>
    </section>
  );
}

function ExplorePathsSection() {
  return (
    <AcquisitionLinkGrid
      title="Explore Char by workflow, platform, or alternative"
      description="These are the highest-intent paths on the site: team workflows, meeting platform guides, and direct comparisons against the tools people switch from."
      className="px-4 pt-16 pb-8"
      items={[
        {
          eyebrow: "Solutions",
          title: "Browse team workflows",
          description:
            "Start with the use cases Char is already built around, from sales to research to developer-heavy teams.",
          href: "/solutions/",
        },
        {
          eyebrow: "Solutions",
          title: "AI meeting notes for sales",
          description:
            "See how Char supports revenue teams with searchable notes, summaries, and fewer meeting follow-up gaps.",
          href: "/solution/sales",
        },
        {
          eyebrow: "Solutions",
          title: "Char for developers",
          description:
            "Open source, local-first, and flexible enough for teams that want to inspect and extend the stack.",
          href: "/solution/engineering",
        },
        {
          eyebrow: "Integrations",
          title: "Browse meeting platform guides",
          description:
            "See how Char works with Zoom, Google Meet, Microsoft Teams, and Webex without meeting bots.",
          href: "/integrations/",
        },
        {
          eyebrow: "Integrations",
          title: "Zoom AI notetaker guide",
          description:
            "Read the Zoom-specific landing page for note capture, transcription, and bot-free workflows.",
          href: "/integrations/zoom/notetaker",
        },
        {
          eyebrow: "Comparisons",
          title: "Compare Char vs Otter",
          description:
            "See the control, privacy, and workflow differences that make Char a stronger fit for high-agency teams.",
          href: "/vs/otter",
        },
      ]}
    />
  );
}

function FAQSection() {
  return (
    <section id="faq" className="px-4 pt-16 pb-16">
      <div className="mx-auto flex flex-col gap-4 md:flex-row md:gap-8">
        <div className="mb-4 text-left md:mb-12">
          <h2 className="text-color mb-4 font-mono text-2xl tracking-wide md:text-4xl">
            Frequently Asked Questions
          </h2>
        </div>

        <FAQ>
          <FAQItem question="What languages does Char support?">
            45+ languages including English, Spanish, French, German, Japanese,
            Mandarin, and more.
          </FAQItem>

          <FAQItem question="Can I import existing recordings?">
            Yes. Upload audio files or transcripts to turn them into searchable,
            summarized notes.
          </FAQItem>

          <FAQItem question="Does Char train AI models on my data?">
            No. Char does not use your recordings, transcripts, or notes to
            train AI models. When using cloud providers, your data is processed
            according to their privacy policies, but Char itself never collects
            or uses your data for training.
          </FAQItem>

          <FAQItem question="Is Char safe?">
            Char doesn't store your conversations. Every meeting audio,
            transcript, and note is a file on your computer. You decide if your
            data ever leaves your device.
          </FAQItem>

          <FAQItem question="How is Char different from other AI note-takers?">
            Plain markdown files instead of proprietary databases. System audio
            capture instead of meeting bots. Your choice of AI provider instead
            of vendor lock-in. Open source instead of a black box.
          </FAQItem>
        </FAQ>
      </div>
    </section>
  );
}

function BlogSection() {
  const sortedArticles = [...allArticles]
    .sort((a, b) => {
      const aDate = a.date;
      const bDate = b.date;
      return new Date(bDate).getTime() - new Date(aDate).getTime();
    })
    .slice(0, 3);

  if (sortedArticles.length === 0) {
    return null;
  }

  return (
    <section id="blog" className="py-16">
      <div className="border-color-brand mb-12 border-b px-4 pb-8 text-left">
        <h2 className="text-color mb-2 font-mono text-2xl tracking-wide md:text-4xl">
          Latest from our blog
        </h2>
        <p className="text-fg-muted font-base">
          Insights, updates, and stories from the Char team
        </p>
        <div className="mt-4 text-left">
          <Link
            to="/blog/"
            className="text-fg-muted hover:text-color inline-flex items-center gap-2 font-medium transition-colors"
          >
            View all articles
            <svg
              xmlns="http://www.w3.org/2000/svg"
              fill="none"
              viewBox="0 0 24 24"
              strokeWidth="2"
              stroke="currentColor"
              className="h-4 w-4"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M13.5 4.5 21 12m0 0-7.5 7.5M21 12H3"
              />
            </svg>
          </Link>
        </div>
      </div>

      <div className="grid gap-4 px-4 md:grid-cols-3">
        {sortedArticles.map((article) => {
          return (
            <Link
              key={article._meta.filePath}
              to="/blog/$slug/"
              params={{ slug: article.slug }}
              className="group block h-full"
            >
              <article className="bg-surface border-color-brand flex h-full flex-col overflow-hidden rounded-md border p-2 transition-all duration-300 hover:shadow-lg">
                <div className="flex flex-1 flex-col px-2 pt-4">
                  <h3 className="text-color text-fg mb-2 line-clamp-2 font-mono text-xl font-medium">
                    {article.display_title || article.meta_title}
                  </h3>

                  <p className="text-fg-muted text-fg mb-4 line-clamp-3 flex-1 text-base leading-relaxed">
                    {article.meta_description}
                  </p>

                  <div className="flex items-center justify-between gap-4 py-4">
                    <time
                      dateTime={article.date}
                      className="text-fg-muted text-xs"
                    >
                      {new Date(article.date).toLocaleDateString("en-US", {
                        month: "short",
                        day: "numeric",
                        year: "numeric",
                      })}
                    </time>

                    <span className="text-fg-muted group-hover:text-fg-muted text-xs font-medium transition-colors">
                      Read →
                    </span>
                  </div>
                </div>
              </article>
            </Link>
          );
        })}
      </div>
    </section>
  );
}
