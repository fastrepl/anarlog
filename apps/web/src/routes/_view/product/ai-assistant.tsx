import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import {
  AnimatePresence,
  motion,
  useMotionValue,
  useTransform,
} from "motion/react";
import { useEffect, useRef, useState } from "react";

import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { cn } from "@hypr/utils";

import {
  JiraToolCall,
  SearchToolCall,
  TranscriptToolCall,
} from "@/components/ai-feature-panel";
import { MockChatInput } from "@/components/mock-chat-input";
import { MockWindow } from "@/components/mock-window";

export const Route = createFileRoute("/_view/product/ai-assistant")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Chat - Char" },
      {
        name: "description",
        content:
          "AI assistant that helps you before, during, and after meetings. Prepare with research, get realtime insights, and execute workflows—all powered by local AI.",
      },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
});

const FEATURES = [
  {
    title: "Ask about past conversations",
    description:
      "Query your entire conversation history to refresh your memory. Find decisions, action items, or specific topics discussed in previous meetings\u2014all in natural language.",
  },
  {
    title: "Execute workflows and tasks",
    description:
      "Describe what you want to do, and let your AI assistant handle the rest. Automate follow-up tasks across your tools without manual data entry.",
    integrations: [
      { icon: "simple-icons:slack", label: "" },
      { icon: "simple-icons:linear", label: "" },
      { icon: "simple-icons:jira", label: "" },
    ],
  },
  {
    title: "Chat during meetings",
    description:
      "Get instant answers from the current transcript and past meeting context.",
  },
  {
    title: "Improve with every transcription",
    description:
      "Your AI assistant learns from every interaction, adapting to your preferences and continuously improving transcription accuracy and summary quality.",
  },
  {
    title: "Deep Research based on your meetings",
    description:
      "Search through past conversations, extract key insights, and understand context before you join.",
  },
];

function Component() {
  return (
    <div className="min-h-screen">
      <div className="mx-auto">
        <HeroSection />
        <ScrollFeatureSection />
        <ExtensionsSection />
        <TemplatesSection />
        <HowItWorksSection />
        <CTASection />
      </div>
    </div>
  );
}

function HeroSection() {
  return (
    <div className="h-2/3 bg-linear-to-b from-stone-50/30 to-stone-100/30 py-12 lg:py-20">
      <header className="mx-auto max-w-4xl px-4 text-left">
        <h1 className="mb-6 flex flex-wrap items-center justify-center font-mono text-4xl tracking-tight text-stone-600 sm:text-5xl">
          <span>AI Chat</span>
          <img
            src="/api/images/hyprnote/ai-assistant.gif"
            alt="AI Chat"
            className="mr-3 ml-1 inline-block size-16 rounded-full object-cover sm:mr-0"
          />
          <span>for your meetings</span>
        </h1>
        <p className="text-lg text-neutral-600 sm:text-xl md:pb-16">
          Prepare, engage, and follow through with AI-powered assistance
        </p>
        <div className="flex justify-center pt-24">
          <MockChatInput />
        </div>
      </header>
    </div>
  );
}

const HEADER_HEIGHT = 69;

function ScrollFeatureSection() {
  const containerRef = useRef<HTMLDivElement>(null);
  const pinnedRef = useRef<HTMLDivElement>(null);
  const [activeIndex, setActiveIndex] = useState(0);
  const scrollProgress = useMotionValue(0);

  useEffect(() => {
    const onScroll = () => {
      const container = containerRef.current;
      const pinned = pinnedRef.current;
      if (!container || !pinned) return;

      const rect = container.getBoundingClientRect();
      const viewH = window.innerHeight - HEADER_HEIGHT;
      const containerH = container.offsetHeight;

      const scrolledPast = HEADER_HEIGHT - rect.top;
      const maxScroll = containerH - viewH;

      if (scrolledPast <= 0) {
        pinned.style.position = "absolute";
        pinned.style.top = "0px";
        pinned.style.bottom = "auto";
        pinned.style.left = "0";
        pinned.style.right = "0";
        pinned.style.width = "";
        pinned.style.height = `${viewH}px`;
      } else if (scrolledPast >= maxScroll) {
        pinned.style.position = "absolute";
        pinned.style.top = "auto";
        pinned.style.bottom = "0px";
        pinned.style.left = "0";
        pinned.style.right = "0";
        pinned.style.width = "";
        pinned.style.height = `${viewH}px`;
      } else {
        pinned.style.position = "fixed";
        pinned.style.top = `${HEADER_HEIGHT}px`;
        pinned.style.bottom = "auto";
        pinned.style.left = `${rect.left}px`;
        pinned.style.right = "auto";
        pinned.style.width = `${container.offsetWidth}px`;
        pinned.style.height = `${viewH}px`;
      }

      const progress = Math.max(0, Math.min(1, scrolledPast / maxScroll));
      const index = Math.min(
        Math.floor(progress * FEATURES.length),
        FEATURES.length - 1,
      );
      setActiveIndex(index);
      scrollProgress.set(progress);
    };

    window.addEventListener("scroll", onScroll, { passive: true });
    window.addEventListener("resize", onScroll, { passive: true });
    onScroll();
    return () => {
      window.removeEventListener("scroll", onScroll);
      window.removeEventListener("resize", onScroll);
    };
  });

  const scrollToFeature = (index: number) => {
    const container = containerRef.current;
    if (!container) return;

    const containerTop = container.getBoundingClientRect().top + window.scrollY;
    const containerH = container.offsetHeight;
    const viewH = window.innerHeight - HEADER_HEIGHT;
    const maxScroll = containerH - viewH;
    const segmentSize = maxScroll / FEATURES.length;
    const targetScroll =
      containerTop -
      HEADER_HEIGHT +
      (index / FEATURES.length) * maxScroll +
      segmentSize * 0.1;

    window.scrollTo({ top: targetScroll, behavior: "smooth" });
  };

  return (
    <>
      {/* Desktop */}
      <div
        ref={containerRef}
        className="relative hidden border-t border-neutral-100 md:block"
        style={{ height: `${FEATURES.length * 100}vh` }}
      >
        <div ref={pinnedRef} className="grid grid-cols-2 bg-white">
          <div className="flex flex-col justify-center border-r border-neutral-100">
            <div className="flex h-full flex-col">
              {FEATURES.map((feature, index) => (
                <motion.div
                  key={feature.title}
                  animate={{
                    opacity: index === activeIndex ? 1 : 0.35,
                  }}
                  transition={{ duration: 0.4, ease: "easeOut" }}
                  className="relative h-1/5 cursor-pointer overflow-hidden border-b border-neutral-100"
                  onClick={() => scrollToFeature(index)}
                >
                  <FeatureProgressBar
                    index={index}
                    activeIndex={activeIndex}
                    scrollProgress={scrollProgress}
                    total={FEATURES.length}
                  />
                  <div className="relative z-10 px-4 py-6 lg:px-8">
                    <h3 className="mb-1 font-mono text-xl text-stone-600">
                      {feature.title}
                    </h3>
                    <p
                      className={cn([
                        "text-sm leading-relaxed text-neutral-500 transition-colors duration-400",
                        index === activeIndex && "text-neutral-600",
                      ])}
                    >
                      {feature.description}
                    </p>
                    {feature.integrations && (
                      <div className="mt-3 flex items-center gap-3">
                        {feature.integrations.map((item) => (
                          <div
                            key={item.label}
                            className="flex items-center gap-1.5 text-xs text-neutral-400"
                          >
                            <Icon icon={item.icon} className="text-sm" />
                            <span>{item.label}</span>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                </motion.div>
              ))}
            </div>
          </div>

          <div className="flex items-center justify-center p-12 lg:p-16">
            <FeatureVisual activeIndex={activeIndex} />
          </div>
        </div>
      </div>

      {/* Mobile */}
      <div className="md:hidden">
        {FEATURES.map((feature, index) => (
          <div
            key={feature.title}
            className={cn([
              "border-b border-neutral-100 px-6 py-10",
              index === 0 && "border-t",
            ])}
          >
            <h3 className="mb-3 font-mono text-lg text-stone-600">
              {feature.title}
            </h3>
            <p className="mb-6 leading-relaxed text-neutral-600">
              {feature.description}
            </p>
            <div className="flex justify-center">
              <FeatureVisual activeIndex={index} />
            </div>
          </div>
        ))}
      </div>
    </>
  );
}

function FeatureProgressBar({
  index,
  activeIndex,
  scrollProgress,
  total,
}: {
  index: number;
  activeIndex: number;
  scrollProgress: ReturnType<typeof useMotionValue<number>>;
  total: number;
}) {
  const segmentStart = index / total;
  const segmentEnd = (index + 1) / total;

  const scaleX = useTransform(
    scrollProgress,
    [segmentStart, segmentEnd],
    [0, 1],
  );

  const isActive = index === activeIndex;
  const isPast = index < activeIndex;

  return (
    <div className="absolute inset-0 overflow-hidden">
      {isPast ? (
        <div className="absolute inset-0" />
      ) : isActive ? (
        <motion.div
          className="absolute inset-0 origin-left"
          style={{ scaleX }}
        />
      ) : null}
    </div>
  );
}

type ChatStep = {
  node: React.ReactNode | ((activeIndex: number) => React.ReactNode);
  delay: number;
};

type ChatPanel = {
  type: "chat";
  steps: ChatStep[];
  footer?: React.ReactNode;
};

type SpecialPanel = {
  type: "special";
  content: React.ReactNode;
};

type Panel = ChatPanel | SpecialPanel;

const CHAT_PANELS: Panel[] = [
  {
    type: "chat",
    steps: [
      {
        delay: 200,
        node: (
          <div className="flex w-full justify-end">
            <div className="w-2/3 rounded-t-2xl rounded-bl-2xl border border-neutral-200 bg-blue-50 px-4 py-3">
              <p className="text-sm text-stone-700">
                What did Sarah say about the timeline?
              </p>
            </div>
          </div>
        ),
      },
      {
        delay: 800,
        node: (idx: number) => <SearchToolCall loopKey={idx} />,
      },
      {
        delay: 3000,
        node: (
          <div className="w-2/3 rounded-xl border border-stone-200 bg-gradient-to-b from-white to-stone-100 px-4 py-3">
            <p className="mb-1 text-sm text-stone-500">Char</p>
            <p className="text-sm text-stone-700">
              In your Oct 12 meeting, Sarah mentioned the deadline is Q1 2026
              with a soft launch in December.
            </p>
          </div>
        ),
      },
    ],
  },
  {
    type: "chat",
    steps: [
      {
        delay: 200,
        node: (
          <div className="flex w-full justify-end">
            <div className="w-2/3 rounded-t-2xl rounded-bl-2xl border border-neutral-200 bg-blue-50 px-4 py-3">
              <p className="text-sm text-stone-700">
                Create a Jira ticket for the mobile bug and assign to Sarah
              </p>
            </div>
          </div>
        ),
      },
      {
        delay: 800,
        node: (idx: number) => <JiraToolCall loopKey={idx} />,
      },
      {
        delay: 3200,
        node: (
          <div className="w-2/3 rounded-xl border border-stone-200 bg-gradient-to-b from-white to-stone-100 px-4 py-3">
            <p className="mb-1 text-sm text-stone-500">Char</p>
            <div className="flex items-center gap-2 text-sm">
              <Icon
                icon="mdi:check-circle"
                className="text-sm text-green-500"
              />
              <span className="text-stone-700">
                Jira ticket ENG-247 created and assigned to Sarah.
              </span>
            </div>
          </div>
        ),
      },
    ],
  },
  {
    type: "chat",
    steps: [
      {
        delay: 200,
        node: (
          <div className="flex w-full justify-end">
            <div className="w-2/3 rounded-t-2xl rounded-bl-2xl border border-neutral-200 bg-blue-50 px-4 py-3">
              <p className="text-sm text-stone-700">
                What's the timeline for the mobile UI?
              </p>
            </div>
          </div>
        ),
      },
      {
        delay: 800,
        node: (idx: number) => <TranscriptToolCall loopKey={idx} />,
      },
      {
        delay: 2800,
        node: (
          <div className="w-2/3 rounded-xl border border-stone-200 bg-gradient-to-b from-white to-stone-100 px-4 py-3">
            <p className="mb-1 text-sm text-stone-500">Char</p>
            <p className="text-sm text-stone-700">
              Ben committed to auth module this week. Sarah estimates 2 sprints
              for full API.
            </p>
          </div>
        ),
      },
    ],
    footer: (
      <div>
        <div className="h-[2px] w-full bg-red-400" />
        <div className="flex items-center justify-between px-3 py-2">
          <div className="flex items-center gap-2">
            <div className="size-2 animate-pulse rounded-full bg-red-400" />
            <span className="text-xs text-neutral-500">Design weekly sync</span>
          </div>
          <div className="flex items-center gap-2">
            <button className="flex size-8 items-center justify-center rounded-full bg-red-500 text-white transition-colors hover:bg-red-600">
              <Icon icon="mdi:phone-hangup" className="text-sm" />
            </button>
            <button className="flex size-8 items-center justify-center rounded-full bg-neutral-100 text-neutral-500 transition-colors hover:bg-neutral-200">
              <Icon icon="mdi:dots-horizontal" className="text-base" />
            </button>
          </div>
        </div>
      </div>
    ),
  },
  {
    type: "special",
    content: (
      <div className="flex flex-col gap-4 p-4">
        <div className="mb-2 flex items-center gap-2">
          <Icon icon="mdi:trending-up" className="text-lg text-purple-500" />
          <span className="text-sm font-medium text-stone-600">
            Quality improving over time
          </span>
        </div>
        <div className="rounded-xl border border-neutral-200 bg-neutral-50 px-4 py-3">
          <p className="mb-1 text-xs text-neutral-400">Before</p>
          <p className="text-sm text-neutral-500 line-through decoration-neutral-300">
            the team talked about doing stuff with the dashboard and some api
            things
          </p>
        </div>
        <div className="rounded-xl border border-stone-200 bg-stone-50 px-4 py-3">
          <p className="mb-1 text-xs text-stone-400">After</p>
          <p className="text-sm text-stone-700">
            The team agreed to prioritize the dashboard redesign and begin API
            migration in Sprint 14.
          </p>
        </div>
        <div className="flex gap-4 pt-2">
          {[
            { label: "Accuracy", value: "94%" },
            { label: "Adapted", value: "12x" },
          ].map((stat) => (
            <div key={stat.label} className="text-sm">
              <span className="font-medium text-stone-700">{stat.value}</span>
              <span className="ml-1 text-neutral-400">{stat.label}</span>
            </div>
          ))}
        </div>
      </div>
    ),
  },
  {
    type: "special",
    content: (
      <div className="flex flex-col gap-4 p-4">
        <div className="rounded-xl border border-neutral-200 bg-white p-3">
          <div className="mb-2 flex items-center gap-3">
            <div className="flex size-8 items-center justify-center rounded-full bg-stone-200 text-xs text-stone-600">
              JK
            </div>
            <div>
              <p className="text-sm font-medium text-stone-700">Jennifer Kim</p>
              <p className="text-xs text-neutral-500">Product Manager</p>
            </div>
          </div>
          <div className="mt-1 flex flex-wrap gap-1.5">
            {["Q4 roadmap", "Mobile launch", "Budget review"].map((t) => (
              <span
                key={t}
                className="rounded-full border border-amber-200 bg-amber-50 px-2 py-0.5 text-[11px] text-amber-700"
              >
                {t}
              </span>
            ))}
          </div>
        </div>
        <div className="rounded-xl border border-stone-200 bg-stone-50 px-4 py-3">
          <p className="text-sm text-stone-700">
            Last 3 meetings focused on mobile launch timeline. Jennifer prefers
            concise bullet-point summaries.
          </p>
        </div>
        <div className="flex items-center gap-2 pt-1">
          <Icon
            icon="mdi:file-search-outline"
            className="text-sm text-amber-500"
          />
          <span className="text-xs text-neutral-500">
            5 past meetings analyzed
          </span>
        </div>
      </div>
    ),
  },
];

function ChatMessages({
  panel,
  activeIndex,
}: {
  panel: ChatPanel;
  activeIndex: number;
}) {
  const [visibleCount, setVisibleCount] = useState(0);

  useEffect(() => {
    setVisibleCount(0);
    const timers = panel.steps.map((step, i) =>
      setTimeout(() => setVisibleCount(i + 1), step.delay),
    );
    return () => timers.forEach(clearTimeout);
  }, [activeIndex, panel.steps]);

  return (
    <motion.div
      className="flex min-h-[280px] flex-col justify-end gap-3 p-4"
      exit={{ opacity: 0, y: -24, filter: "blur(8px)" }}
      transition={{ duration: 0.25, ease: "easeIn" }}
    >
      <AnimatePresence initial={false}>
        {panel.steps.slice(0, visibleCount).map((step, i) => (
          <motion.div
            key={`${activeIndex}-${i}`}
            initial={{ opacity: 0, y: 16 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -16, filter: "blur(6px)" }}
            transition={{ duration: 0.4, ease: "easeOut" }}
            layout
          >
            {typeof step.node === "function"
              ? step.node(activeIndex)
              : step.node}
          </motion.div>
        ))}
      </AnimatePresence>
    </motion.div>
  );
}

function FeatureVisual({ activeIndex }: { activeIndex: number }) {
  const [inputValue, setInputValue] = useState("");
  const panel = CHAT_PANELS[activeIndex];
  const isChat = panel.type === "chat";
  const hasFooter = isChat && !!panel.footer;

  return (
    <div className="w-full max-w-[420px]">
      <motion.div
        layout
        transition={{ layout: { duration: 0.35, ease: "easeInOut" } }}
      >
        <AnimatePresence mode="wait">
          {isChat ? (
            <ChatMessages
              key={`chat-${activeIndex}`}
              panel={panel}
              activeIndex={activeIndex}
            />
          ) : (
            <motion.div
              key={`special-${activeIndex}`}
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -20, filter: "blur(8px)" }}
              transition={{ duration: 0.4, ease: "easeOut" }}
            >
              {panel.content}
            </motion.div>
          )}
        </AnimatePresence>

        <AnimatePresence mode="wait">
          {isChat && (
            <motion.div
              key={`footer-${hasFooter ? "custom" : "input"}`}
              initial={{ opacity: 0, height: 0, filter: "blur(8px)" }}
              animate={{ opacity: 1, height: "auto", filter: "blur(0px)" }}
              exit={{ opacity: 0, height: 0, filter: "blur(8px)" }}
              transition={{ duration: 0.4, ease: "easeInOut" }}
              className="overflow-hidden"
            >
              {hasFooter ? (
                panel.footer
              ) : (
                <div className="rounded-2xl border border-neutral-100 bg-gradient-to-b from-stone-50 to-stone-100 p-3">
                  <div className="flex items-center gap-2">
                    <input
                      type="text"
                      value={inputValue}
                      onChange={(e) => setInputValue(e.target.value)}
                      placeholder="Ask Char anything..."
                      className="flex-1 bg-transparent text-sm text-stone-700 outline-none placeholder:text-neutral-400"
                    />
                    <div
                      className={cn([
                        "flex size-7 shrink-0 items-center justify-center rounded-lg transition-colors",
                        inputValue
                          ? "bg-stone-600 text-white"
                          : "bg-neutral-100 text-neutral-300",
                      ])}
                    >
                      <Icon icon="mdi:arrow-up" className="text-base" />
                    </div>
                  </div>
                </div>
              )}
            </motion.div>
          )}
        </AnimatePresence>
      </motion.div>
    </div>
  );
}

function ExtensionsSection() {
  return (
    <section>
      <div className="flex flex-col items-center justify-center border-t border-neutral-100">
        <div className="p-8 pt-16">
          <h2 className="pb-8 text-left font-mono text-2xl tracking-wide text-stone-600 md:text-4xl">
            Realtime insights via{" "}
            <Link
              to="/product/extensions/"
              className="text-stone-600 underline decoration-dotted underline-offset-2 hover:text-stone-800"
            >
              extensions
            </Link>
          </h2>
          <p className="mb-4 max-w-3xl text-left leading-relaxed text-neutral-600">
            AI-powered extensions provide live assistance during your meeting.
            Built on our extension framework, these tools adapt to your needs in
            realtime.
          </p>
          <div className="mt-6 text-left">
            <Link
              to="/product/extensions/"
              className="inline-flex items-center gap-2 font-medium text-stone-600 hover:text-stone-800"
            >
              Learn more about extensions
              <Icon icon="mdi:arrow-right" className="text-lg" />
            </Link>
          </div>
        </div>

        <div className="">
          <div className="px-6 py-8 lg:px-8">
            <p className="text-md mb-6 text-left text-stone-400">
              Available realtime extensions
            </p>
            <div className="grid gap-6 md:grid-cols-3">
              <div className="rounded-lg border border-neutral-200 bg-stone-50 p-6">
                <Icon
                  icon="mdi:comment-check"
                  className="mb-3 text-2xl text-stone-600"
                />
                <h5 className="mb-2 font-medium text-stone-700">Suggestions</h5>
                <p className="text-sm text-neutral-600">
                  Get AI-generated advice and recommendations based on the
                  conversation flow
                </p>
              </div>

              <div className="rounded-lg border border-neutral-200 bg-stone-50 p-6">
                <Icon
                  icon="mdi:account-voice"
                  className="mb-3 text-2xl text-stone-600"
                />
                <h5 className="mb-2 font-medium text-stone-700">
                  Talk time tracking
                </h5>
                <p className="text-sm text-neutral-600">
                  Monitor who's speaking and for how long to ensure balanced
                  participation
                </p>
              </div>

              <div className="rounded-lg border border-neutral-200 bg-stone-50 p-6">
                <Icon
                  icon="mdi:school"
                  className="mb-3 text-2xl text-stone-600"
                />
                <h5 className="mb-2 font-medium text-stone-700">
                  ELI5 explanations
                </h5>
                <p className="text-sm text-neutral-600">
                  Get instant simple explanations of technical or professional
                  jargon
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

const TEMPLATE_PROMPTS = [
  "Create a customer discovery template",
  "Generate questions for a technical interview",
  "Build an agenda for our quarterly review",
];

function TemplatesSection() {
  return (
    <section className="px-6 pt-16 pb-16 lg:px-8">
      <div className="mx-auto mb-12 max-w-3xl text-left">
        <h2 className="pb-8 text-left font-mono text-2xl tracking-wide text-stone-600 md:text-4xl">
          Generate custom templates
        </h2>
        <p className="leading-relaxed text-neutral-600">
          Create tailored meeting templates on the spot. Ask your AI assistant
          to generate agendas, question lists, or note structures specific to
          your meeting type.
        </p>
      </div>

      <div className="mx-auto grid max-w-4xl gap-4 md:grid-cols-3">
        {TEMPLATE_PROMPTS.map((prompt) => (
          <div
            key={prompt}
            className={cn([
              "rounded-t-2xl rounded-bl-2xl border border-neutral-200 bg-gradient-to-b from-stone-50 to-stone-100",
              "px-5 py-4 text-sm text-stone-600",
            ])}
          >
            {prompt}
          </div>
        ))}
      </div>
    </section>
  );
}

function HowItWorksSection() {
  const [typedText1, setTypedText1] = useState("");
  const [typedText2, setTypedText2] = useState("");
  const [enhancedLines, setEnhancedLines] = useState(0);
  const [activeTab, setActiveTab] = useState<
    "notes" | "summary" | "transcription"
  >("notes");

  const text1 = "metrisc w/ john";
  const text2 = "stakehlder mtg";

  useEffect(() => {
    const runAnimation = () => {
      setTypedText1("");
      setTypedText2("");
      setEnhancedLines(0);
      setActiveTab("notes");

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
                  setActiveTab("summary");

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
                                setTimeout(() => runAnimation(), 2000);
                              }, 800);
                            }, 800);
                          }, 800);
                        }, 800);
                      }, 800);
                    }, 800);
                  }, 300);
                }, 800);
              }
            }, 50);
          }
        }, 50);
      }, 500);
    };

    runAnimation();
  }, []);

  return (
    <section>
      <div className="flex flex-col items-center gap-4 px-4 pt-16 pb-8 text-left">
        <h2 className="font-mono text-2xl tracking-wide text-neutral-700 md:text-4xl">
          How Char works
        </h2>
        <p className="text-md mx-auto max-w-xl pb-4 text-neutral-500">
          We believe that file is more important than software. All saves
          locally, in plain markdown
          <span className="text-neutral-400">.md</span>
        </p>
      </div>
      <div className="flex justify-center overflow-hidden px-4 pt-8 sm:px-8 sm:pt-16">
        <MockWindow
          showAudioIndicator={activeTab === "notes"}
          audioIndicatorColor="#ef4444"
          headerClassName={activeTab === "notes" ? "bg-red-100" : undefined}
        >
          <div className="flex border-b border-neutral-200 text-sm">
            {(["notes", "summary", "transcription"] as const).map((tab) => (
              <button
                key={tab}
                onClick={() => setActiveTab(tab)}
                className={cn([
                  "px-4 py-2 transition-colors",
                  activeTab === tab
                    ? "border-b-2 border-neutral-900 text-neutral-900"
                    : "text-neutral-400 hover:text-neutral-600",
                ])}
              >
                {tab === "notes"
                  ? "Your Notes"
                  : tab === "summary"
                    ? "Summary"
                    : "Transcription"}
              </button>
            ))}
          </div>

          <div className="h-75 overflow-hidden p-6">
            {activeTab === "notes" && (
              <div>
                <div className="text-neutral-700">ui update - moble</div>
                <div className="text-neutral-700">api</div>
                <div className="mt-4 text-neutral-700">new dash - urgnet</div>
                <div className="text-neutral-700">a/b tst next wk</div>
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
            )}

            {activeTab === "summary" && (
              <div className="flex flex-col gap-4">
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
                      streamlined navigation bar and improved button placements
                      for better accessibility.
                    </li>
                    <li
                      className={cn([
                        "transition-opacity duration-500",
                        enhancedLines >= 3 ? "opacity-100" : "opacity-0",
                      ])}
                    >
                      Ben confirmed that API adjustments are needed to support
                      dynamic UI changes, particularly for fetching personalized
                      user data more efficiently.
                    </li>
                    <li
                      className={cn([
                        "transition-opacity duration-500",
                        enhancedLines >= 4 ? "opacity-100" : "opacity-0",
                      ])}
                    >
                      The UI update will be implemented in phases, starting with
                      core navigation improvements. Ben will ensure API
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
                      Alice emphasized that the new analytics dashboard must be
                      prioritized due to increasing stakeholder demand.
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
            )}

            {activeTab === "transcription" && (
              <div className="flex flex-col gap-3 text-sm">
                <div className="text-neutral-700">
                  <span className="font-medium text-neutral-900">Sarah:</span>{" "}
                  So the mobile UI update is looking good. We've streamlined the
                  nav bar and improved button placements.
                </div>
                <div className="text-neutral-700">
                  <span className="font-medium text-neutral-900">Ben:</span>{" "}
                  I'll need to adjust the API to support dynamic UI changes,
                  especially for personalized user data.
                </div>
                <div className="text-neutral-700">
                  <span className="font-medium text-neutral-900">Alice:</span>{" "}
                  The new dashboard is urgent. Stakeholders have been asking
                  about it every day.
                </div>
                <div className="text-neutral-700">
                  <span className="font-medium text-neutral-900">Mark:</span> We
                  should align the dashboard launch with our marketing push next
                  quarter.
                </div>
              </div>
            )}
          </div>
        </MockWindow>
      </div>

      <div className="flex flex-col divide-y divide-neutral-200 border-t border-neutral-200 *:w-full *:px-8 *:pt-8 *:pb-8 md:h-80 md:flex-row md:divide-x md:*:w-1/3 *:lg:pt-16">
        <div className="flex flex-col justify-between">
          <div className="justify-left flex h-20 items-center gap-6">
            <Icon icon="mdi:wifi-off" className="text-2xl text-stone-600" />
            <div className="item-center flex rounded-md border border-red-300 bg-red-100 px-2 py-2">
              <DancingSticks
                amplitude={1}
                height={24}
                width={120}
                color="#ef4444"
              />
            </div>
          </div>
          <div className="flex flex-col gap-2 md:gap-4">
            <h4 className="font-mono text-lg text-stone-700">
              Use local models or use Your Own key
            </h4>
            <p className="text-md text-neutral-500">
              Char works with various transcription models right on your device,
              even without internet.
            </p>
          </div>
        </div>

        <div className="group flex flex-col justify-between gap-8 sm:gap-4">
          <div className="justify-left flex items-center gap-6 md:min-h-20">
            <div className="relative flex h-16 w-full items-center justify-center rounded-md border-2 border-dashed border-green-300 px-2 py-2 lg:h-full">
              <div className="flex h-16 w-16 items-center justify-center rounded-full bg-gray-100">
                <Icon
                  icon="mdi:file-upload"
                  className="text-2xl text-stone-600"
                />
              </div>

              <div className="absolute flex rotate-8 flex-row items-center gap-2 rounded-md border border-gray-300 bg-gray-100 py-2 pr-4 pl-2 text-nowrap shadow-lg transition-all duration-500 ease-out group-hover:translate-x-[10%] group-hover:-translate-y-[50%] group-hover:translate-y-[10%] group-hover:rotate-6 lg:right-1/4 lg:bottom-1/4 lg:translate-x-[5%] lg:-translate-y-[5%]">
                <img
                  src="/icons/grabbed-cursor.svg"
                  className="absolute top-1/2 left-1/2 h-8 w-8"
                  alt=""
                />
                <Icon
                  icon="mdi:file-outline"
                  className="text-2xl text-stone-600"
                />
                <div className="flex flex-col">
                  <p className="text-md text-neutral-600">
                    Meeting.12.03.26-11.32.wav
                  </p>
                  <p className="text-sm text-neutral-300">14:30:25</p>
                </div>
              </div>
            </div>
          </div>
          <div className="flex flex-col gap-2 md:gap-4">
            <h4 className="font-mono text-lg text-stone-700">
              Upload records or existing transcripts
            </h4>
            <p className="text-md text-neutral-500">
              Drag and drop audio files or paste existing transcripts to
              generate summaries instantly.
            </p>
          </div>
        </div>

        <div className="flex flex-col justify-between gap-8 sm:gap-4">
          <div className="flex items-center">
            <div className="flex w-full items-center justify-between gap-2 rounded-xl border border-neutral-200 bg-gradient-to-t from-white to-stone-100 px-4 py-3 text-nowrap shadow-lg md:gap-3">
              <div className="flex items-center gap-2 md:gap-3">
                <Icon
                  icon="mdi:video"
                  className="shrink-0 text-xl text-stone-500"
                />
                <div className="flex flex-col gap-0.5">
                  <p className="text-xs text-neutral-400">1-1 with Joanna</p>
                  <p className="text-sm font-medium text-neutral-500">
                    AI Notetaker joined the call.
                  </p>
                </div>
              </div>
              <button className="ml-2 shrink-0 text-neutral-300 transition-colors hover:text-neutral-500">
                <Icon icon="mdi:close" className="text-base" />
              </button>
            </div>
          </div>

          <div className="flex flex-col gap-2 md:gap-4">
            <h4 className="font-mono text-lg text-stone-700">
              No bot on calls
            </h4>
            <p className="text-md text-neutral-500">
              Char connects right to your system audio and captures every word
              perfectly, no faceless bots join your meetings.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}

function CTASection() {
  return (
    <section className="bg-linear-to-t from-stone-50/30 to-stone-100/30 px-4 py-16 lg:px-0">
      <div className="flex flex-col items-center gap-6 text-left">
        <div className="mb-4 flex size-40 items-center justify-center rounded-[48px] border border-neutral-100 bg-transparent shadow-2xl">
          <img
            src="/api/images/hyprnote/icon.png"
            alt="Char"
            width={144}
            height={144}
            className="mx-auto size-36 rounded-[40px] border border-neutral-100"
          />
        </div>
        <h2 className="font-mono text-2xl sm:text-3xl">
          Start using your AI assistant
        </h2>
        <p className="mx-auto max-w-2xl text-lg text-neutral-600">
          Get AI-powered help before, during, and after every meeting with Char
        </p>
        <div className="flex flex-col items-center justify-center gap-4 pt-6 sm:flex-row">
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
          <Link
            to="/product/ai-notetaking/"
            className={cn([
              "flex h-12 items-center justify-center px-6 text-base sm:text-lg",
              "rounded-full border border-neutral-300 text-stone-600",
              "transition-colors hover:bg-white",
            ])}
          >
            Learn about AI Notetaking
          </Link>
        </div>
      </div>
    </section>
  );
}
