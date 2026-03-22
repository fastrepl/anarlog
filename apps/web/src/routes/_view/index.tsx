import { Icon } from "@iconify-icon/react";
import MuxPlayer, { type MuxPlayerRefAttributes } from "@mux/mux-player-react";
import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { allArticles } from "content-collections";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useId, useRef, useState } from "react";

import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { cn } from "@hypr/utils";

import {
  JiraToolCall,
  TranscriptToolCall,
} from "@/components/ai-feature-panel";
import { AppPreviewSection } from "@/components/app-preview";
import { DownloadButton } from "@/components/download-button";
import { GithubStars } from "@/components/github-stars";
import { Image } from "@/components/image";
import { LogoCloud } from "@/components/logo-cloud";
import { FAQ, FAQItem } from "@/components/mdx-jobs";
import { MockChatInput } from "@/components/mock-chat-input";
import { NotebookGrid } from "@/components/notebook-grid";
import { SocialCard } from "@/components/social-card";
import { VideoModal } from "@/components/video-modal";
import { addContact } from "@/functions/loops";
import { useHeroContext } from "@/hooks/use-hero-context";
import { getHeroCTA, getPlatformCTA, usePlatform } from "@/hooks/use-platform";
import { useAnalytics } from "@/hooks/use-posthog";

const MUX_PLAYBACK_ID = "bpcBHf4Qv5FbhwWD02zyFDb24EBuEuTPHKFUrZEktULQ";

const heroContent = {
  title: "AI Notepad for Meetings \u2014 No Strings Attached.",
  subtitle: "No forced cloud. No data held hostage. No bots in your meetings.",
  valueProps: [
    {
      title: "Zero lock-in",
      description:
        "Choose your preferred STT and LLM provider. Cloud or local.",
    },
    {
      title: "You own your data",
      description: "Plain markdown files on your device. Works with any tool.",
    },
    {
      title: "Just works",
      description:
        "A simple, familiar notepad, real-time transcription, and AI summaries.",
    },
  ],
};

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

const socialProofRedditBody = `Dear Hyprnote Team,

I wanted to take a moment to commend you on the impressive work you've done with Hyprnote. Your commitment to privacy, on-device AI, and transparency is truly refreshing in today's software landscape. The fact that all transcription and summarization happens locally and live!—without compromising data security—makes Hyprnote a standout solution, especially for those of us in compliance-sensitive environments.

The live transcription is key for me. It saves a landmark step to transcribe each note myself using macwhisper. Much more handy they way you all do this. The Calendar function is cool too.

I am a telephysician and my notes are much more quickly done. Seeing 6-8 patients daily and tested it yesteday. So yes, my job is session heavy. Add to that being in psychiatry where document making sessions become voluminous, my flow is AI dependent to make reports stand out. Accuracy is key for patient care.

Hyprnote is now part of that process.

Thank you for your dedication and for building a tool that not only saves time, but also gives peace of mind. I look forward to seeing Hyprnote continue to evolve

Cheers!`;

const socialProofLinkedInBody = `Guys at Hyprnote (YC S25) are wild.

Had a call with John Jeong about their product (privacy-first AI notepad).

Next day? They already shipped a first version of the context feature we discussed 🤯

24 𝐡𝐨𝐮𝐫𝐬. A conversation turned into production

As Product Engineer at Waveful, where we also prioritize rapid execution, I deeply respect this level of speed.

The ability to ship this fast while maintaining quality, is what separates great teams from the rest 🔥

Btw give an eye to Hyprnote:
100% local AI processing
Zero cloud dependency
Real privacy
Almost daily releases

Their repo: https://lnkd.in/dKCtxkA3 (mac only rn but they're releasing for windows very soon)

Been using it for daily tasks, even simple note-taking is GREAT because I can review everything late, make action points etc.

Mad respect to the team. This is how you build in 2025. 🚀`;

export const Route = createFileRoute("/_view/")({
  component: Component,
});

function Component() {
  const [expandedVideo, setExpandedVideo] = useState<string | null>(null);
  const heroInputRef = useRef<HTMLInputElement>(null);

  return (
    <main className="min-h-screen flex-1 overflow-x-hidden">
      <div className="mx-auto">
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
        <SocialTestimonialsSection />
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
      <div className="flex w-full min-w-0 flex-col text-left">
        <section
          id="hero"
          className="isolate flex w-full min-w-0 overflow-visible px-4 pt-10 text-left"
        >
          <div className="border-brand-bright items-left relative z-10 flex min-h-[80vh] w-full min-w-0 flex-row content-between rounded-lg border">
            <div className="flex flex-col justify-between px-6 pt-8 pb-8 md:pt-12 md:pr-8 md:pb-12 md:pl-12">
              <div className="flex flex-col gap-6">
                <h1 className="text-color text-2xl break-words sm:text-6xl">
                  {heroContent.title}
                </h1>
                <p className="font-regular text-fg-muted text-base leading-relaxed break-words sm:text-xl">
                  {heroContent.subtitle}
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
                  <div className="flex w-full flex-col items-stretch gap-2 md:items-start">
                    <DownloadButton />
                  </div>
                )}
              </div>

              <div className="flex flex-col gap-3 md:gap-6">
                {heroContent.valueProps.map((prop) => (
                  <p
                    key={prop.title}
                    className="text-fg-muted text-sm md:text-base"
                  >
                    {prop.description}
                  </p>
                ))}
                {heroCTA.subtextLink ? (
                  <Link
                    to={heroCTA.subtextLink}
                    className="text-fg-muted hover:text-color hidden text-base underline transition-colors md:block"
                  >
                    {heroCTA.subtext}
                  </Link>
                ) : (
                  <p className="text-fg/50 hidden text-sm md:block">
                    {heroCTA.subtext}
                  </p>
                )}
              </div>
            </div>

            <div className="relative hidden w-1/2 shrink-0 self-stretch overflow-hidden p-8 md:block">
              <NotebookGrid />
              <div className="absolute right-0 bottom-0 flex justify-end p-10">
                <button
                  onClick={() => onVideoExpand(MUX_PLAYBACK_ID)}
                  className="group relative w-4/5 overflow-hidden rounded-xl border border-neutral-200 shadow-xl"
                  style={{ aspectRatio: "16/9" }}
                >
                  <img
                    src={`https://image.mux.com/${MUX_PLAYBACK_ID}/thumbnail.jpg?width=1280&height=720&fit_mode=smartcrop`}
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
                </button>
              </div>
            </div>
          </div>
        </section>

        {/* <div className="relative aspect-video w-full max-w-4xl overflow-hidden border-t border-neutral-100 md:hidden">
          <VideoThumbnail
            playbackId={MUX_PLAYBACK_ID}
            onPlay={() => onVideoExpand(MUX_PLAYBACK_ID)}
          />
        </div>

        <div className="w-full">
          <ValuePropsGrid valueProps={heroContent.valueProps} />
          <div className="relative hidden aspect-video w-full overflow-hidden border-t border-neutral-100 md:block">
            <VideoThumbnail
              playbackId={MUX_PLAYBACK_ID}
              onPlay={() => onVideoExpand(MUX_PLAYBACK_ID)}
            />
          </div>
        </div> */}
      </div>
    </div>
  );
}

function LogoSection() {
  return (
    <section className="px-4 py-24">
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
      <h2 className="text-color mb-10 font-mono text-2xl tracking-wide md:text-4xl">
        What people are saying
      </h2>

      <div className="flex flex-col gap-6 md:hidden">
        <SocialCard
          platform="reddit"
          author="spilledcarryout"
          subreddit="macapps"
          body={socialProofRedditBody}
          url="https://www.reddit.com/r/macapps/comments/1lo24b9/comment/n15dr0t/"
        />
        <SocialCard
          platform="linkedin"
          author="Flavius Catalin Miron"
          role="Product Engineer"
          company="Waveful"
          body={socialProofLinkedInBody}
          url="https://www.linkedin.com/in/flaviews/"
        />
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
          body="I love the flexibility that @tryhyprnote gives me to integrate personal notes with AI summaries. I can quickly jot down important points during the meeting without getting distracted, then trust that the AI will capture them in full detail for review afterwards."
          url="https://twitter.com/tomyang11_/status/1956395933538902092"
        />
      </div>

      <div className="hidden gap-8 md:grid md:grid-cols-3">
        <SocialCard
          platform="reddit"
          author="spilledcarryout"
          subreddit="macapps"
          body={socialProofRedditBody}
          url="https://www.reddit.com/r/macapps/comments/1lo24b9/comment/n15dr0t/"
        />
        <SocialCard
          platform="linkedin"
          author="Flavius Catalin Miron"
          role="Product Engineer"
          company="Waveful"
          body={socialProofLinkedInBody}
          url="https://www.linkedin.com/in/flaviews/"
        />
        <div className="flex flex-col gap-8">
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
  const agentWorkflowGraphicId = useId().replaceAll(":", "");
  const [enhancedLines, setEnhancedLines] = useState(0);

  useEffect(() => {
    const runAnimation = () => {
      setEnhancedLines(0);

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
      }, 800);
    };

    runAnimation();
  }, []);

  return (
    <section id="how-it-works" className="px-4 pt-8 pb-24">
      <div className="flex flex-col">
        {/* Header */}
        <div className="border-brand-color border-b py-10">
          <h2 className="text-color font-mono text-2xl leading-relaxed tracking-wide md:text-5xl">
            Focus on conversation <br /> while Char makes notes
          </h2>
        </div>

        {/* Block 1: Listen & Write */}
        <div className="flex flex-col md:flex-row">
          <div className="flex flex-col justify-end gap-4 pr-8 pb-16 md:w-1/2">
            <p className="font-regular text-color text-lg leading-relaxed md:text-3xl">
              Char keeps track of everything that happens during the meeting,
              includes context about previous conversations and people you talk
              to.
            </p>
          </div>

          <div className="bg-lined-notebook md:w-1/2">
            <motion.div
              initial="hidden"
              whileInView="visible"
              viewport={{ once: true, amount: 0.3 }}
              variants={{
                hidden: {},
                visible: { transition: { staggerChildren: 0.3 } },
              }}
              className="flex flex-col gap-4 p-8"
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
                    <span className="absolute inline-flex size-full animate-ping rounded-full bg-red-400 opacity-75" />
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
                  className="border-brand-color bg-surface h-[200px] w-full overflow-hidden rounded-xl border sm:h-[300px] sm:w-1/2"
                >
                  <div className="border-brand-color bg-surface-subtle relative flex h-[38px] shrink-0 items-center gap-2 border-b px-4">
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
                    <div className="text-color overflow-hidden text-base whitespace-pre-line">
                      {"ui update - moble\napi\nnew dash - urgnet"}
                      <span className="animate-pulse text-xl text-blue-700">
                        |
                      </span>
                    </div>
                  </div>
                </motion.div>
                <motion.div
                  variants={{
                    hidden: {},
                    visible: { transition: { staggerChildren: 0.15 } },
                  }}
                  className="grid w-full grid-cols-2 place-content-around gap-4 sm:w-1/2"
                >
                  {["design weekly.md", "1:1 with John.md", "Q2 goals.md"].map(
                    (name, i) => (
                      <motion.div
                        key={name}
                        variants={{
                          hidden: { opacity: 0, y: -10 },
                          visible: {
                            opacity: 1,
                            y: 0,
                            transition: { duration: 0.4, ease: "easeOut" },
                          },
                        }}
                        className="bg-surface border-brand-color relative h-32 w-full rounded border"
                        style={{
                          clipPath:
                            "polygon(0 0, calc(100% - 24px) 0, 100% 24px, 100% 100%, 0 100%)",
                          transform: `rotate(${[-3, 2, -5][i]}deg)`,
                        }}
                      >
                        <div className="bg-brand absolute top-0 right-0 h-[24px] w-[24px]" />
                        <p className="text-fg absolute right-3 bottom-2 text-sm">
                          {name}
                        </p>
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
                        name: "Ben J.",
                        color: "bg-red-200 border-red-300 text-red-500",
                      },
                      {
                        name: "Sarah M.",
                        color: "bg-blue-200 border-blue-300 text-blue-500",
                      },
                      {
                        name: "Victor F.",
                        color: "bg-amber-200 border-amber-300 text-amber-500",
                      },
                    ].map(({ name, color }) => (
                      <div
                        key={name}
                        className="flex items-center gap-2 rounded-full border border-stone-200 bg-stone-50 px-1.5 py-1"
                      >
                        <div
                          className={cn([
                            "flex size-5 items-center justify-center rounded-full border text-sm font-bold",
                            color,
                          ])}
                        >
                          {name[0]}
                        </div>
                        <span className="text-fg-muted pr-1.5 text-sm font-medium">
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
          <div className="bg-lined-notebook flex flex-col justify-center px-8 md:w-1/2">
            <DotWaveTransition />
          </div>
        </div>

        {/* Block 2: Summarize */}
        <div className="-mt-px flex flex-col md:flex-row">
          <div className="flex flex-col justify-start gap-4 pt-16 pr-8 md:w-1/2">
            <p className="text-color text-lg leading-relaxed md:text-3xl">
              After the meeting, Char combines your notes with transcripts to
              create a perfect summary.
            </p>
          </div>

          <div className="bg-lined-notebook flex-1">
            <motion.div
              initial={{ opacity: 0, y: -20 }}
              whileInView={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.6, ease: "easeOut" }}
              viewport={{ once: true, amount: 0.6 }}
              className="flex h-full items-end justify-center p-8"
            >
              <div className="surface border-brand-color w-full overflow-hidden rounded-xl border">
                <div className="border-brand-color bg-surface-subtle relative flex h-[38px] items-center gap-2 border-b px-4">
                  <div className="flex gap-2">
                    <div className="size-3 rounded-full bg-red-400" />
                    <div className="size-3 rounded-full bg-yellow-400" />
                    <div className="size-3 rounded-full bg-green-400" />
                  </div>
                </div>
                <div className="flex w-full flex-col gap-4 overflow-hidden p-6">
                  <div className="flex flex-col gap-2">
                    <h4
                      className={cn([
                        "text-color text-lg font-semibold transition-opacity duration-500",
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
                    <ul className="text-color flex list-disc flex-col gap-2 pl-5 text-sm">
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
              </div>
            </motion.div>
          </div>
        </div>

        {/* features block */}
        <div className="flex snap-x snap-mandatory gap-4 overflow-x-auto pt-16 pb-4 [scrollbar-width:none] md:grid md:grid-cols-5 md:gap-12 md:overflow-visible md:pb-0 md:*:min-h-[320px] md:*:py-8">
          {/* local or cloud */}
          <div className="border-brand flex min-w-[260px] shrink-0 snap-center flex-col justify-between gap-2 md:min-w-0 md:shrink">
            <div className="flex h-full items-center gap-4">
              <Icon icon="mdi:wifi-off" className="text-fg-muted text-2xl" />
              <div className="flex rounded-md border border-red-300 bg-red-100 px-2 py-2">
                <DancingSticks
                  amplitude={1}
                  height={24}
                  width={100}
                  color="#ef4444"
                />
              </div>
            </div>
            <div className="flex min-h-0 flex-col justify-end gap-2 md:min-h-[200px]">
              <h4 className="text-color mb-4 font-mono text-base font-medium md:text-xl">
                Local or cloud, your choice
              </h4>
              <p className="text-color text-base">
                Use local models or bring your own API key. Works without
                internet.
              </p>
            </div>
          </div>

          {/* use any agent */}
          <div className="group border-brand flex min-w-[260px] shrink-0 snap-center flex-col justify-between md:min-w-0 md:shrink">
            <div className="flex w-full min-w-0 items-stretch gap-1 sm:gap-3">
              <div className="flex flex-1 flex-col justify-between">
                <div className="flex size-8 shrink-0 items-center justify-center self-start rounded-full">
                  <img
                    src="/icons/Chatgpt-logo.svg"
                    alt=""
                    className="size-7 object-contain"
                  />
                </div>
                <div className="bg-surface-subtle flex size-8 shrink-0 items-center justify-center self-end rounded-full">
                  <img
                    src="/icons/Claude-logo.svg"
                    alt=""
                    className="size-7 object-contain"
                  />
                </div>
                <div className="bg-surface-subtle flex size-8 shrink-0 items-center justify-center self-start rounded-full">
                  <img
                    src="/icons/gemini%20logo.svg"
                    alt=""
                    className="size-7 object-contain"
                  />
                </div>
              </div>
              {/* Folder */}
              <div className="flex shrink-0 items-center justify-center self-stretch px-1">
                <svg
                  className="aspect-[162/134] h-14 w-auto"
                  viewBox="0 0 162 134"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                >
                  <path
                    d="M0 17.5918V116.408C0 122.566 0 125.645 1.19837 127.997C2.25248 130.066 3.93449 131.748 6.0033 132.802C8.35524 134 11.4341 134 17.5918 134H142.865C149.022 134 152.101 134 154.453 132.802C156.522 131.748 158.204 130.066 159.258 127.997C160.456 125.645 160.456 122.566 160.456 116.408V29.969C160.456 23.8113 160.456 20.7324 159.258 18.3805C158.204 16.3117 156.522 14.6297 154.453 13.5755C152.101 12.3772 149.022 12.3772 142.865 12.3772H69.6948C66.9975 12.3772 65.6488 12.3772 64.3801 12.0717C63.2552 11.8009 62.1802 11.3542 61.1946 10.7481C60.083 10.0646 59.1314 9.10884 57.2283 7.19737L55.2196 5.1798L55.2195 5.17975C53.3164 3.26831 52.3649 2.31258 51.2532 1.62902C50.2677 1.02299 49.1926 0.576311 48.0678 0.305478C46.799 0 45.4504 0 42.753 0H17.5918C11.4341 0 8.35524 0 6.0033 1.19837C3.93449 2.25248 2.25248 3.93449 1.19837 6.0033C0 8.35524 0 11.4341 0 17.5918Z"
                    fill={`url(#paint0-${agentWorkflowGraphicId})`}
                  />
                  <path
                    d="M3.95117 25.7577V123.36C3.95117 126.959 6.86882 129.877 10.4679 129.877H149.988C153.587 129.877 156.505 126.959 156.505 123.36V25.7577C156.505 22.1586 153.587 19.241 149.988 19.241H10.4679C6.86881 19.241 3.95117 22.1586 3.95117 25.7577Z"
                    fill="white"
                  />
                  <g filter={`url(#filter0-${agentWorkflowGraphicId})`}>
                    <path
                      d="M0 43.7046V116.236C0 122.394 0 125.473 1.19837 127.825C2.25248 129.894 3.93449 131.576 6.0033 132.63C8.35524 133.828 11.4341 133.828 17.5918 133.828H142.865C149.022 133.828 152.101 133.828 154.453 132.63C156.522 131.576 158.204 129.894 159.258 127.825C160.456 125.473 160.456 122.394 160.456 116.236V43.7046C160.456 37.5469 160.456 34.468 159.258 32.1161C158.204 30.0473 156.522 28.3653 154.453 27.3112C152.101 26.1128 149.022 26.1128 142.865 26.1128H17.5918C11.4341 26.1128 8.35524 26.1128 6.0033 27.3112C3.93449 28.3653 2.25248 30.0473 1.19837 32.1161C0 34.468 0 37.5469 0 43.7046Z"
                      fill={`url(#paint1-${agentWorkflowGraphicId})`}
                    />
                    <path
                      d="M0 43.7046V116.236C0 122.394 0 125.473 1.19837 127.825C2.25248 129.894 3.93449 131.576 6.0033 132.63C8.35524 133.828 11.4341 133.828 17.5918 133.828H142.865C149.022 133.828 152.101 133.828 154.453 132.63C156.522 131.576 158.204 129.894 159.258 127.825C160.456 125.473 160.456 122.394 160.456 116.236V43.7046C160.456 37.5469 160.456 34.468 159.258 32.1161C158.204 30.0473 156.522 28.3653 154.453 27.3112C152.101 26.1128 149.022 26.1128 142.865 26.1128H17.5918C11.4341 26.1128 8.35524 26.1128 6.0033 27.3112C3.93449 28.3653 2.25248 30.0473 1.19837 32.1161C0 34.468 0 37.5469 0 43.7046Z"
                      fill="white"
                      fillOpacity={0.2}
                      style={{ mixBlendMode: "multiply" }}
                    />
                  </g>
                  <g style={{ mixBlendMode: "overlay" }}>
                    <path
                      d="M69.4606 62.907C69.4606 65.3925 68.1634 67.6632 66.5467 69.5959C64.1646 72.4435 62.717 76.0863 62.717 80.0565C62.717 84.0267 64.1647 87.6693 66.5467 90.5169C68.1634 92.4496 69.4606 94.7202 69.4606 97.2057V104.025H61.3927V96.0795C61.3927 93.4736 60.0779 91.0314 57.8701 89.5361L56.5178 88.6204V71.2748L57.87 70.359C60.0779 68.8638 61.3927 66.4216 61.3927 63.8157V56.0884L69.4606 56.0884V62.907Z"
                      fill="var(--color-fg)"
                    />
                    <path
                      d="M91.5113 62.907C91.5113 65.3925 92.8086 67.6632 94.4253 69.5959C96.8073 72.4435 98.2549 76.0863 98.2549 80.0565C98.2549 84.0267 96.8073 87.6693 94.4253 90.5169C92.8085 92.4496 91.5113 94.7202 91.5113 97.2057V104.025H99.5793V96.0795C99.5793 93.4736 100.894 91.0314 103.102 89.5361L104.454 88.6204V71.2748L103.102 70.359C100.894 68.8638 99.5793 66.4216 99.5793 63.8157V56.0884L91.5113 56.0884V62.907Z"
                      fill="var(--color-fg)"
                    />
                  </g>
                  <defs>
                    <filter
                      id={`filter0-${agentWorkflowGraphicId}`}
                      x="0"
                      y="26.1128"
                      width="160.457"
                      height="108.403"
                      filterUnits="userSpaceOnUse"
                      colorInterpolationFilters="sRGB"
                    >
                      <feFlood floodOpacity={0} result="BackgroundImageFix" />
                      <feBlend
                        mode="normal"
                        in="SourceGraphic"
                        in2="BackgroundImageFix"
                        result="shape"
                      />
                      <feColorMatrix
                        in="SourceAlpha"
                        type="matrix"
                        values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0"
                        result="hardAlpha"
                      />
                      <feOffset dy="0.68718" />
                      <feGaussianBlur stdDeviation="0.34359" />
                      <feComposite
                        in2="hardAlpha"
                        operator="arithmetic"
                        k2="-1"
                        k3="1"
                      />
                      <feColorMatrix
                        type="matrix"
                        values="0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0.25 0"
                      />
                      <feBlend
                        mode="normal"
                        in2="shape"
                        result="effect1_innerShadow"
                      />
                    </filter>
                    <linearGradient
                      id={`paint0-${agentWorkflowGraphicId}`}
                      x1="82.9667"
                      y1="4.12572"
                      x2="82.9667"
                      y2="134.075"
                      gradientUnits="userSpaceOnUse"
                    >
                      <stop stopColor="#2BC5F4" />
                      <stop offset="0.190196" stopColor="#00A7DE" />
                    </linearGradient>
                    <linearGradient
                      id={`paint1-${agentWorkflowGraphicId}`}
                      x1="91.3745"
                      y1="16.4999"
                      x2="91.3745"
                      y2="133.828"
                      gradientUnits="userSpaceOnUse"
                    >
                      <stop stopColor="#49D2FC" />
                      <stop offset="0.5" stopColor="#00B4E7" />
                      <stop offset="1" stopColor="#00B4E7" />
                    </linearGradient>
                  </defs>
                </svg>
              </div>
              <div className="flex flex-1 flex-col justify-between">
                <div className="bg-surface-subtle flex size-8 shrink-0 items-center justify-center self-end rounded-full">
                  <img
                    src="/icons/Obsidian%20logo.svg"
                    alt=""
                    className="size-7 object-contain"
                  />
                </div>
                <div className="bg-surface-subtle flex size-8 shrink-0 items-center justify-center self-start rounded-full">
                  <img
                    src="/icons/Open-claw%20logo.svg"
                    alt=""
                    className="size-7 object-contain"
                  />
                </div>
                <div className="bg-surface-subtle flex size-8 shrink-0 items-center justify-center self-end rounded-full">
                  <img
                    src="/icons/manus%20logo.svg"
                    alt=""
                    className="size-7 object-contain"
                  />
                </div>
              </div>
            </div>
            <div className="flex min-h-0 flex-col justify-end gap-2 md:min-h-[200px]">
              <h4 className="text-color mb-4 font-mono text-base font-medium md:text-xl">
                Create <br /> any workflow
              </h4>
              <p className="text-color text-base">
                Char is fully avaliable to any agent because of it's
                markdown-first nature
              </p>
            </div>
          </div>

          {/* no bot on calls */}
          <div className="border-brand flex min-w-[260px] shrink-0 snap-center flex-col justify-between md:min-w-0 md:shrink">
            <div className="flex h-full items-center">
              <div className="flex h-16 max-w-90 justify-center">
                <div className="border-brand-color flex w-full items-center justify-between gap-2 rounded-xl border bg-gradient-to-b from-white to-stone-100 px-4 py-3 text-nowrap shadow-lg">
                  <div className="flex items-center gap-2">
                    <Icon
                      icon="mdi:video"
                      className="text-fg-muted shrink-0 text-xl"
                    />
                    <div className="flex flex-col gap-0.5">
                      <p className="text-fg-subtle text-xs">1-1 with Joanna</p>
                      <p className="text-fg-muted text-sm font-medium">
                        AI Notetaker joined.
                      </p>
                    </div>
                  </div>

                  <button className="text-fg-subtle hover:text-fg-muted ml-2 shrink-0 transition-colors">
                    <Icon icon="mdi:close" className="text-base" />
                  </button>
                </div>
              </div>
            </div>
            <div className="flex min-h-0 flex-col justify-end gap-2 md:min-h-[200px]">
              <h4 className="text-color mb-4 font-mono text-base font-medium md:text-xl">
                Meetings without bots
              </h4>
              <p className="text-color text-base">
                Char captures system audio directly. No faceless bots join your
                meetings.
              </p>
            </div>
          </div>

          {/* upload existing recordings */}
          <div className="border-brand flex min-w-[260px] shrink-0 snap-center flex-col justify-between md:min-w-0 md:shrink">
            <div className="flex h-full items-center">
              <div className="relative flex h-16 w-full items-center justify-center rounded-lg border-2 border-dashed border-green-300 bg-green-100 px-2 py-2">
                <div className="flex size-10 items-center justify-center rounded-full bg-gray-100">
                  <Icon
                    icon="mdi:file-upload"
                    className="text-fg-muted text-xl"
                  />
                </div>
                <div className="border-brand-color surface absolute flex rotate-8 flex-row items-center gap-2 rounded-md border py-3 pr-4 pl-2 text-nowrap shadow-lg lg:right-1/4 lg:bottom-1/4 lg:translate-x-[5%] lg:-translate-y-[5%]">
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
                      fill-rule="evenodd"
                      clip-rule="evenodd"
                      d="M8.58243 2.64649C9.68243 2.23399 11.8595 2.48608 12.4324 3.72358C13.0053 4.96108 13.3491 6.56524 13.372 6.17566C13.3282 4.99155 13.4282 3.8065 13.6699 2.64649C13.9246 1.90357 14.5083 1.31996 15.2512 1.06524C15.9325 0.849761 16.6559 0.802581 17.3595 0.927743C18.0709 1.07418 18.7009 1.4833 19.1241 2.07358C19.6602 3.40992 19.9625 4.82851 20.0178 6.26733C20.0748 5.03958 20.2827 3.8235 20.6366 2.64649C21.0195 2.10692 21.5788 1.71789 22.2178 1.54649C22.9755 1.40797 23.7519 1.40797 24.5095 1.54649C25.1314 1.75288 25.6753 2.14475 26.0678 2.66941C26.5546 3.88434 26.8484 5.16789 26.9387 6.47358C26.9387 6.79441 27.0991 5.57983 27.6033 4.77774C28.0083 3.57537 29.3113 2.92898 30.5137 3.33399C31.716 3.739 32.3624 5.04204 31.9574 6.24441C31.9574 7.73399 31.9574 7.66524 31.9574 8.67358C31.9574 9.68191 31.9574 10.5757 31.9574 11.4236C31.8749 12.7647 31.691 14.0977 31.4074 15.4111C31.0097 16.5737 30.4545 17.6763 29.7574 18.6882C28.645 19.9258 27.7256 21.3242 27.0303 22.8361C26.8607 23.5878 26.7838 24.3574 26.8012 25.1277C26.7989 25.8396 26.8914 26.5486 27.0762 27.2361C26.1393 27.3362 25.1943 27.3362 24.2574 27.2361C23.3637 27.0986 22.2637 25.3111 21.9658 24.7611C21.8184 24.4658 21.5167 24.2792 21.1866 24.2792C20.8565 24.2792 20.5548 24.4658 20.4074 24.7611C19.9033 25.6319 18.7803 27.2132 18.1158 27.3048C16.5803 27.4882 13.3949 27.3048 10.9199 27.3048C10.9199 27.3048 11.3553 25.0132 10.3928 24.1882C9.43034 23.3632 8.49076 22.4007 7.78034 21.759L5.87826 19.6507C4.53693 18.4055 3.55538 16.8224 3.03659 15.0673C2.55534 12.9132 2.60117 11.8819 3.03659 11.0111C3.48069 10.292 4.17416 9.76167 4.98451 9.52149C5.65773 9.39937 6.35076 9.44662 7.00117 9.65899C7.45095 9.84729 7.83967 10.1567 8.12409 10.5527C8.65117 11.2632 8.83451 11.6069 8.60534 10.8277C8.37618 10.0486 7.87201 9.47566 7.61992 8.53608C7.12917 7.42645 6.83453 6.24013 6.74909 5.02983C6.79595 3.92807 7.52955 2.97439 8.58243 2.64649Z"
                      stroke="black"
                      stroke-width="1.71875"
                      stroke-linejoin="round"
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
            <div className="flex min-h-0 flex-col justify-end gap-2 md:min-h-[200px]">
              <h4 className="text-color mb-4 font-mono text-base font-medium md:text-xl">
                Upload existing recordings
              </h4>
              <p className="text-color text-base">
                Drop in audio files or transcripts to turn them into searchable
                notes.
              </p>
            </div>
          </div>

          {/* languages */}
          <div className="border-brand flex min-w-[260px] shrink-0 snap-center flex-col justify-between md:min-w-0 md:shrink">
            <div className="flex h-full items-center justify-center p-8">
              <HelloBubble />
            </div>
            <div className="flex min-h-0 flex-col justify-end gap-2 md:min-h-[200px]">
              <h4 className="text-color mb-4 font-mono text-base font-medium md:text-xl">
                Transcribe over 40+ languages
              </h4>
              <p className="text-color text-base">
                We use best in class models and constantly updates them to
                support new languages.
              </p>
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
      <div className="border-brand-color w-2/3 rounded-t-2xl rounded-bl-2xl border bg-blue-50 px-4 py-3">
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

  useEffect(() => {
    const id = setInterval(() => {
      setIndex((i) => (i + 1) % helloWords.length);
    }, 2000);
    return () => clearInterval(id);
  }, []);

  const current = helloWords[index];

  return (
    <AnimatePresence mode="wait">
      <motion.div
        key={current.text}
        initial={{ opacity: 0, scale: 0.8, filter: "blur(4px)" }}
        animate={{ opacity: 1, scale: 1, filter: "blur(0px)" }}
        exit={{ opacity: 0, scale: 0.8, filter: "blur(4px)" }}
        transition={{ duration: 0.3, ease: "easeInOut" }}
        className="rounded-full rounded-bl-sm bg-blue-500 px-6 py-3"
      >
        <span className="block text-2xl font-medium whitespace-nowrap text-white">
          {current.text}
        </span>
      </motion.div>
    </AnimatePresence>
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
    <div className="border-brand-color w-2/3 rounded-t-xl rounded-br-xl border bg-gradient-to-b from-white to-stone-100 px-4 py-3">
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

function ChatInput() {
  return (
    <div className="p-3">
      <div className="flex items-center gap-2 rounded-xl border border-neutral-200 bg-white px-3 py-2">
        <span className="text-fg-subtle flex-1 text-sm">
          Ask Char anything...
        </span>
        <div className="text-fg-subtle flex size-6 shrink-0 items-center justify-center rounded-lg bg-neutral-100">
          <Icon icon="mdi:arrow-up" className="text-xs" />
        </div>
      </div>
    </div>
  );
}

function WorkflowGraphic() {
  const [step, setStep] = useState(0);

  useEffect(() => {
    const t1 = setTimeout(() => setStep(1), 200);
    const t2 = setTimeout(() => setStep(2), 800);
    const t3 = setTimeout(() => setStep(3), 3200);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
      clearTimeout(t3);
    };
  }, []);

  return (
    <div className="flex w-full max-w-[420px] flex-col">
      <div className="flex h-[280px] flex-col justify-end gap-3">
        <AnimatePresence initial={false}>
          {step >= 1 && (
            <motion.div
              key="q"
              initial={{ opacity: 0, y: 16 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.4, ease: "easeOut" }}
            >
              <ChatBubbleQuestion text="Create a Jira ticket for the mobile bug and assign to Sarah" />
            </motion.div>
          )}
          {step >= 2 && (
            <motion.div
              key="tool"
              initial={{ opacity: 0, y: 16 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.4, ease: "easeOut" }}
            >
              <JiraToolCall loopKey={0} />
            </motion.div>
          )}
          {step >= 3 && (
            <motion.div
              key="r"
              initial={{ opacity: 0, y: 16 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.4, ease: "easeOut" }}
            >
              <ChatBubbleResponse
                text="Jira ticket ENG-247 created and assigned to Sarah."
                withCheck
              />
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}

function LiveGraphic() {
  return (
    <div className="flex w-full flex-col gap-3">
      <div className="flex w-full items-center justify-between rounded-full bg-stone-700 p-2 pl-6">
        <div className="flex items-center gap-3">
          <div className="relative flex size-3">
            <span className="absolute inline-flex size-full animate-ping rounded-full bg-red-400 opacity-75" />
            <span className="relative inline-flex size-3 rounded-full bg-red-500" />
          </div>
          <p className="text-sm text-white">Weekly Team Sync</p>
          <span className="text-xs text-white/50">42:17</span>
        </div>
        <div className="flex items-center gap-2">
          <div className="flex items-center justify-center rounded-full px-2">
            <Icon
              icon="mdi:dots-horizontal"
              className="text-xl text-white/60"
            />
          </div>
          <div className="flex items-center justify-center rounded-full bg-red-600 px-3 py-2">
            <Icon icon="mdi:phone-hangup" className="text-xl text-white" />
          </div>
        </div>
      </div>

      <div className="flex flex-col gap-2">
        <ChatBubbleQuestion text="What's the timeline for the mobile UI?" />
        <TranscriptToolCall loopKey={0} static />
        <ChatBubbleResponse text="Ben committed to auth module this week. Sarah estimates 2 sprints for full API." />
      </div>
    </div>
  );
}

export function AISection() {
  return (
    <section id="ai" className="px-4 py-16">
      <div className="items-left flex flex-col gap-4 pb-12 text-left">
        <h2 className="text-fg font-mono text-2xl tracking-wide md:text-4xl">
          Get more from every note with AI
        </h2>
        <p className="text-fg-muted">
          Ask questions, execute tasks, and grow your knowledge base—all from
          your meeting notes.
        </p>
      </div>

      <div className="surface-subtle border-brand-color grid grid-cols-1 gap-px rounded-xl border md:grid-cols-3">
        {/* Block 1: Search */}
        <div className="border-brand-color flex flex-col md:border-r">
          <div className="flex min-h-[240px] flex-col gap-2 p-8">
            <h3 className="text-color mb-3 font-mono text-2xl font-medium">
              Chat with your notes
            </h3>
            <p className="text-color-muted text-base leading-relaxed">
              Query your entire conversation history. Find decisions, action
              items, or topics discussed in previous meetings in natural
              language.
            </p>
          </div>
          <div className="bg-dotted-dark flex min-h-[280px] flex-1 items-end justify-end p-8">
            <MockChatInput
              prompts={[
                "What did Sarah say about the timeline?",
                "Any action items from last week's sync?",
                "What decisions were made in Q1 planning?",
              ]}
              className="w-full"
            />
          </div>
        </div>

        {/* Block 2: Workflow */}
        <div className="border-brand-color flex flex-col md:border-r">
          <div className="flex min-h-[240px] flex-col gap-2 p-8">
            <h3 className="text-color mb-3 font-mono text-2xl font-medium">
              Execute workflows and tasks
            </h3>
            <p className="text-color-muted text-base leading-relaxed">
              Describe what you want to do and let Char handle the rest.
              Automate follow-up tasks across your tools without manual data
              entry.
            </p>
            <div className="flex items-center gap-3 pt-1">
              <Icon
                icon="simple-icons:slack"
                className="text-color-muted text-base"
              />
              <Icon
                icon="simple-icons:linear"
                className="text-color-muted text-base"
              />
              <Icon icon="logos:jira" className="text-fg-subtle text-base" />
            </div>
          </div>
          <div className="bg-dotted-dark flex min-h-[280px] flex-1 items-center justify-center p-8">
            <WorkflowGraphic />
          </div>
        </div>

        {/* Block 3: Live */}
        <div className="flex flex-col">
          <div className="flex min-h-[240px] flex-col gap-2 p-8">
            <h3 className="text-color mb-3 font-mono text-2xl font-medium">
              Chat during live meetings
            </h3>
            <p className="text-color-muted text-base leading-relaxed">
              Get instant answers from the current transcript and past meeting
              context without breaking your flow.
            </p>
          </div>
          <div className="bg-dotted-dark flex min-h-[280px] flex-1 items-center justify-center p-8">
            <LiveGraphic />
          </div>
        </div>
      </div>
    </section>
  );
}

export function GrowsWithYouSection() {
  return (
    <section id="grows-with-you" className="px-4 pt-8 pb-16">
      <div className="surface border-brand-color mx-auto rounded-xl border">
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

        <div className="border-brand-color grid border-t md:grid-cols-2">
          <div className="bg-lined-notebook border-brand-color flex flex-col border-b md:border-r md:border-b-0">
            <div className="flex h-[240px] items-start px-8 pt-8">
              <div className="surface border-brand-color w-full rounded-xl border p-4 md:max-w-2/3">
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
                <div className="border-brand-color bg-surface-subtle rounded border p-3">
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
              <div className="surface-subtle border-brand-color flex w-full items-center justify-between gap-4 rounded-2xl border p-4 md:max-w-2/3">
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

  const handleFeatureIndexChange = useCallback(
    (nextIndex: number) => {
      setSelectedFeature(nextIndex);
      setProgress(0);
      progressRef.current = 0;
    },
    [setSelectedFeature],
  );

  useEffect(() => {
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
  }, [selectedFeature, setSelectedFeature, featuresScrollRef]);

  const handleScrollToFeature = (index: number) => {
    scrollToFeature(index);
    setProgress(0);
    progressRef.current = 0;
  };

  return (
    <section>
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
  const thumbnailUrl = `https://image.mux.com/${playbackId}/thumbnail.jpg?width=1920&height=1080&fit_mode=smartcrop`;

  useEffect(() => {
    const player = playerRef.current;
    if (!player) return;

    if (isActive) {
      player.play()?.catch(() => {
        // Autoplay blocked or player not ready - fail silently
      });
    } else {
      player.pause();
      player.currentTime = 0;
    }
  }, [isActive]);

  return (
    <div className="relative h-full w-full">
      <img
        src={thumbnailUrl}
        alt={alt}
        className={cn([
          "absolute inset-0 h-full w-full object-contain transition-opacity duration-300",
          isActive ? "opacity-0" : "opacity-100",
        ])}
      />
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
  const thumbnailUrl = `https://image.mux.com/${playbackId}/thumbnail.jpg?width=1920&height=1080&fit_mode=smartcrop`;

  useEffect(() => {
    const player = playerRef.current;
    if (!player) return;

    if (isHovered) {
      player.play();
    } else {
      player.pause();
      player.currentTime = 0;
    }
  }, [isHovered]);

  return (
    <div className="relative h-full w-full">
      <img
        src={thumbnailUrl}
        alt={alt}
        className={cn([
          "absolute inset-0 h-full w-full object-contain transition-opacity duration-300",
          isHovered ? "opacity-0" : "opacity-100",
        ])}
      />
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
  sales: { accent: "oklch(0.62 0.1332 49)", bg: "#fefce8", border: "#fde68a" },
  research: {
    accent: "oklch(0.5471 0.1899 264.38)",
    bg: "#eff6ff",
    border: "#bfdbfe",
  },
  legal: { accent: "#374151", bg: "#f9fafb", border: "#d1d5db" },
  engineering: {
    accent: "oklch(0.55 0.2245 292.58)",
    bg: "#f5f3ff",
    border: "#ddd6fe",
  },
  healthcare: {
    accent: "oklch(0.5588 0.1085 165.61)",
    bg: "#ecfdf5",
    border: "#a7f3d0",
  },
  recruiting: {
    accent: "oklch(0.55 0.148 3.96)",
    bg: "#fff1f2",
    border: "#fecdd3",
  },
  "project-management": {
    accent: "oklch(0.6 0.1283 38.4)",
    bg: "#fff7ed",
    border: "#fed7aa",
  },
  journalism: {
    accent: "oklch(0.5059 0.0765 186.39)",
    bg: "#f0fdfa",
    border: "#99f6e4",
  },
};

const solutionScenarios = [
  {
    id: "sales",
    label: "Sales",
    headline: "Close more deals with AI-powered meeting notes",
    description:
      "Stop taking notes during sales calls. Focus on building relationships while Char captures every detail, extracts insights, and prepares your follow-ups.",
    pills: [
      "Capture Every Detail",
      "Deal Intelligence",
      "Action Items",
      "Sales Coaching",
      "Privacy-First",
    ],
    link: "/solution/sales/",
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
  {
    id: "legal",
    label: "Legal",
    headline: "Confidential AI notes for legal professionals",
    description:
      "Capture every client meeting and case discussion with AI that processes everything locally. Your privileged communications stay protected.",
    pills: [
      "Confidentiality First",
      "Accurate Transcription",
      "Case Documentation",
      "Billable Time Tracking",
      "Self-Hosting",
    ],
    link: "/solution/legal",
  },
  {
    id: "engineering",
    label: "Engineering",
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
    id: "healthcare",
    label: "Healthcare",
    headline: "Privacy-first AI notes for healthcare teams",
    description:
      "Capture clinical meetings and patient discussions with AI that processes everything locally. Your patient data never leaves your device.",
    pills: [
      "Privacy-First Design",
      "Clinical Documentation",
      "Care Coordination",
      "Time Savings",
      "Self-Hosting",
    ],
    link: "/solution/healthcare",
  },
  {
    id: "recruiting",
    label: "Recruiting",
    headline: "Hire better with AI-powered interview notes",
    description:
      "Focus on the candidate, not your notepad. Char captures every interview detail so you can make better hiring decisions.",
    pills: [
      "Capture Every Response",
      "Structured Feedback",
      "Objective Comparison",
      "Faster Decisions",
      "Candidate Privacy",
    ],
    link: "/solution/recruiting",
  },
  {
    id: "project-management",
    label: "Project management",
    headline: "Keep projects on track with AI-powered notes",
    description:
      "Focus on leading your projects, not taking notes. Char captures every meeting detail so nothing falls through the cracks.",
    pills: [
      "Action Item Tracking",
      "Stakeholder Management",
      "Status Updates",
      "Risk Documentation",
      "Searchable History",
    ],
    link: "/solution/project-management",
  },
  {
    id: "journalism",
    label: "Journalism",
    headline: "Report with confidence using AI-powered notes",
    description:
      "Focus on asking the right questions while Char captures every quote, verifies accuracy, and helps you tell compelling stories.",
    pills: [
      "Interview Recording",
      "Precise Quotes",
      "Fact Verification",
      "Fast Turnaround",
      "Source Protection",
    ],
    link: "/solution/journalism",
  },
];

function SolutionsTabbar() {
  const [activeId, setActiveId] = useState(solutionScenarios[0].id);
  const active =
    solutionScenarios.find((s) => s.id === activeId) ?? solutionScenarios[0];
  const activeColor = solutionColors[active.id];

  return (
    <section id="solutions" className="px-4 pb-16">
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
                "shrink-0 cursor-pointer px-3 py-3 text-sm font-medium transition-colors hover:pb-6 md:px-4 md:text-lg",
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
            borderRadius: "12px 12px 12px 12px",
          }}
          className="relative z-0 overflow-hidden"
        >
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={activeId}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.12 }}
              className="flex min-h-[400px] flex-col gap-4 px-8 py-16"
            >
              <h3 className="max-w-2xl font-mono text-2xl leading-snug text-white md:text-4xl">
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
              <Link
                to={active.link as "/solution/sales/"}
                className="mt-auto flex items-center gap-1 text-sm text-white underline underline-offset-2 hover:text-white/80"
              >
                Learn more
                <Icon icon="mdi:arrow-top-right" className="text-sm" />
              </Link>
            </motion.div>
          </AnimatePresence>
        </div>
      </div>
    </section>
  );
}

function FAQSection() {
  return (
    <section id="faq" className="px-4 pt-16 pb-16">
      <div className="mx-auto flex flex-col gap-8 md:flex-row md:gap-16">
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
    <section id="blog" className="border-t border-neutral-100 py-16">
      <div className="mb-12 px-4 text-left">
        <h2 className="text-color mb-2 font-mono text-2xl tracking-wide md:text-4xl">
          Latest from our blog
        </h2>
        <p className="text-fg-muted">
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
          const ogImage =
            article.coverImage ||
            `https://char.com/og?type=blog&title=${encodeURIComponent(article.title ?? "")}${article.author.length > 0 ? `&author=${encodeURIComponent(article.author.join(", "))}` : ""}${article.date ? `&date=${encodeURIComponent(new Date(article.date).toLocaleDateString("en-US", { year: "numeric", month: "long", day: "numeric" }))}` : ""}&v=1`;

          return (
            <Link
              key={article._meta.filePath}
              to="/blog/$slug/"
              params={{ slug: article.slug }}
              className="group block h-full"
            >
              <article className="bg-surface border-brand-color flex h-full flex-col overflow-hidden rounded-md border p-2 transition-all duration-300 hover:shadow-lg">
                <div className="bg-surface aspect-40/21 overflow-hidden">
                  <img
                    src={ogImage}
                    alt={article.display_title}
                    className="h-full w-full object-cover transition-all duration-500"
                  />
                </div>

                <div className="flex flex-1 flex-col">
                  {/* <h3 className="text-color group-hover:text-color mb-2 line-clamp-2 font-mono text-xl transition-colors">
                    {article.display_title || article.meta_title}
                  </h3>

                  <p className="text-fg-muted mb-4 line-clamp-3 flex-1 text-base leading-relaxed">
                    {article.meta_description}
                  </p> */}

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

export function CTASection({
  heroInputRef,
}: {
  heroInputRef: React.RefObject<HTMLInputElement | null>;
}) {
  const platform = usePlatform();
  const platformCTA = getPlatformCTA(platform);

  const getButtonLabel = () => {
    if (platform === "mobile") {
      return "Get reminder";
    }
    return platformCTA.label;
  };

  const handleCTAClick = () => {
    if (platformCTA.action === "waitlist") {
      window.scrollTo({ top: 0, behavior: "smooth" });
      setTimeout(() => {
        if (heroInputRef.current) {
          heroInputRef.current.focus();
          heroInputRef.current.parentElement?.classList.add(
            "animate-shake",
            "border-stone-600",
          );
          setTimeout(() => {
            heroInputRef.current?.parentElement?.classList.remove(
              "animate-shake",
              "border-stone-600",
            );
          }, 500);
        }
      }, 500);
    }
  };

  return (
    <section className="laptop:px-0 px-4 py-16">
      <div className="flex flex-col items-center gap-6 text-center">
        <h2 className="text-color font-mono text-2xl tracking-wide md:text-6xl">
          Your meetings. Your data.
          <br className="sm:hidden" /> Your control.
        </h2>
        <p className="text-fg-muted mx-auto max-w-2xl text-lg">
          Start taking meeting notes with AI—without the lock-in
        </p>
        <div className="flex flex-col items-center justify-center gap-4 pt-6 sm:flex-row">
          {platformCTA.action === "download" ? (
            <DownloadButton />
          ) : (
            <button
              onClick={handleCTAClick}
              className={cn([
                "group flex h-12 items-center justify-center px-6 text-base sm:text-lg",
                "rounded-full bg-linear-to-t from-stone-600 to-stone-500 text-white",
                "shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
                "transition-all",
              ])}
            >
              {getButtonLabel()}
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
            </button>
          )}
          <div className="hidden sm:block">
            <GithubStars />
          </div>
        </div>
      </div>
    </section>
  );
}
