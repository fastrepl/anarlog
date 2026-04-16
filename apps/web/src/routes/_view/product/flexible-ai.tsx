import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { cn } from "@hypr/utils";

import { CTASection } from "@/components/cta-section";
import { DownloadButton } from "@/components/download-button";
import { GithubStars } from "@/components/github-stars";
import { FAQ, FAQItem } from "@/components/mdx-shared";

export const Route = createFileRoute("/_view/product/flexible-ai")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Flexible AI - Char" },
      {
        name: "description",
        content:
          "The only AI note-taker that lets you choose your preferred STT and LLM provider. Cloud, BYOK, or fully local.",
      },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
});

const setupOptions = [
  {
    icon: "mdi:cloud-outline",
    eyebrow: "Managed",
    title: "Char Cloud",
    detail: "$8/month",
    description:
      "Start with a setup that works immediately. No API keys, no provider decisions, no configuration drag.",
  },
  {
    icon: "mdi:key-outline",
    eyebrow: "Bring your own stack",
    title: "BYOK",
    detail: "Free",
    description:
      "Use your existing OpenAI, Anthropic, Deepgram, or other provider credits directly without markup.",
  },
  {
    icon: "mdi:laptop-account",
    eyebrow: "Private by default",
    title: "Fully local",
    detail: "On-device",
    description:
      "Run transcription and summaries on your machine when sensitive conversations should never leave it.",
  },
];

const switchBenefits = [
  {
    title: "Start simple, change later",
    description:
      "Begin with Char Cloud, then move to BYOK or local once you know your workflow and constraints.",
  },
  {
    title: "Match the meeting, not the plan",
    description:
      "Use local AI for sensitive calls, cloud models for tougher reasoning, or BYOK when you want cost control.",
  },
  {
    title: "Re-run older notes with better models",
    description:
      "When a stronger model becomes available, process existing transcripts again instead of starting over.",
  },
  {
    title: "Your notes stay put",
    description:
      "The AI layer is flexible, but the notes remain Markdown files on your device either way.",
  },
];

const localCapabilities = [
  {
    icon: "mdi:microphone-outline",
    title: "Local transcription with Whisper",
    description:
      "Download Whisper through Ollama or LM Studio and transcribe meetings without any API calls.",
  },
  {
    icon: "mdi:brain",
    title: "Local summaries and chat",
    description:
      "Run Llama, Mistral, Qwen, or other open models locally for summaries, action items, and question answering.",
  },
];

function Component() {
  return (
    <main className="min-h-screen flex-1 overflow-x-hidden px-2 md:px-8">
      <div className="">
        <HeroSection />
        <AISetupSection />
        <LocalFeaturesSection />
        <SwitchSection />
        <BenchmarkSection />
        <FAQSection />
        <CTASection
          title="Pick the AI setup that fits every meeting"
          description="Start with managed defaults, bring your own providers, or run fully local without switching apps."
        />
      </div>
    </main>
  );
}

function SectionTitle({ children }: { children: ReactNode }) {
  return (
    <div className="text-left">
      <p className="text-color-secondary py-6 font-mono text-xs font-medium tracking-widest uppercase">
        {children}
      </p>
    </div>
  );
}

function HeroSection() {
  return (
    <div className="">
      <div className="px-6 py-12 lg:py-20">
        <header className="mx-auto mb-12 text-left">
          <h1 className="text-color mb-6 font-mono text-2xl tracking-wide sm:text-5xl">
            Take Meeting Notes With
            <br />
            AI of Your Choice
          </h1>
          <p className="text-color text-lg sm:text-xl">
            Char lets you choose between managed cloud AI, your own provider
            keys,
            <br className="hidden sm:inline" /> or fully local models on your
            machine.
          </p>
          <div className="mt-8 flex items-center gap-4">
            <DownloadButton />
            <GithubStars />
          </div>
        </header>
      </div>
    </div>
  );
}

function AISetupSection() {
  return (
    <section className="border-color-brand surface rounded-xl border">
      <div className="border-color-brand border-b px-4 py-6">
        {" "}
        <SectionTitle>Pick your AI setup</SectionTitle>
      </div>
      <div className="grid md:grid-cols-3">
        {setupOptions.map((option, index) => (
          <div
            key={option.title}
            className={cn([
              "border-color-brand p-8",
              index < setupOptions.length - 1 && "md:border-r",
            ])}
          >
            <Icon icon={option.icon} className="text-color mb-4 text-3xl" />
            <p className="text-color-secondary mb-1 font-mono text-xs tracking-widest uppercase">
              {option.eyebrow}
            </p>
            <h3 className="text-color mb-2 font-mono text-xl">
              {option.title}
            </h3>
            <p className="text-color mb-4 text-sm font-medium">
              {option.detail}
            </p>
            <p className="text-fg text-base leading-relaxed">
              {option.description}
            </p>
          </div>
        ))}
      </div>
    </section>
  );
}

function LocalFeaturesSection() {
  return (
    <section className="pt-8">
      <SectionTitle>Local features</SectionTitle>
      <div className="divide-brand flex flex-row divide-x pb-8">
        {localCapabilities.map((capability) => (
          <div key={capability.title} className="flex items-start gap-4 p-8">
            <Icon
              icon={capability.icon}
              className="text-color shrink-0 text-3xl"
            />
            <div>
              <h3 className="text-color mb-2 font-mono text-xl">
                {capability.title}
              </h3>
              <p className="text-fg text-base leading-relaxed">
                {capability.description}
              </p>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function SwitchSection() {
  return (
    <section className="border-color-brand surface rounded-xl border">
      <div className="px-8 pt-6">
        {" "}
        <SectionTitle>Switch providers anytime</SectionTitle>
      </div>
      <p className="text-fg border-color-brand border-b px-8 pb-8 text-left text-base leading-relaxed">
        Your notes are never locked to a single AI provider.
      </p>
      <div className="grid md:grid-cols-2">
        {switchBenefits.map((benefit, index) => (
          <div
            key={benefit.title}
            className={cn([
              "border-color-brand p-8",
              index < 2 && "border-b",
              index % 2 === 0 && "md:border-r",
            ])}
          >
            <h3 className="text-color mb-2 font-mono text-lg font-medium">
              {benefit.title}
            </h3>
            <p className="text-fg text-base leading-relaxed">
              {benefit.description}
            </p>
          </div>
        ))}
      </div>
    </section>
  );
}

function BenchmarkSection() {
  return (
    <section className="px-4 pt-16 pb-16">
      <div className="">
        <h2 className="text-color mb-8 font-mono text-2xl tracking-wide sm:text-3xl">
          Compare model performance before you decide
        </h2>
        <p className="text-fg max-w-2xl text-base leading-relaxed">
          We benchmark leading models on meeting tasks like summaries, action
          items, speaker tracking, and Q&A so you can choose with confidence.
        </p>
      </div>
      <div className="flex flex-col gap-4 pt-10 sm:flex-row">
        <Link
          to="/eval/"
          className={cn([
            "flex h-9 items-center rounded-lg px-4 text-sm transition-colors",
            "border border-neutral-200 bg-white text-neutral-700 hover:bg-neutral-50",
          ])}
        >
          View AI model evaluations
        </Link>
        <Link
          to="/product/local-ai/"
          className={cn([
            "flex h-9 items-center rounded-lg px-4 text-sm transition-colors",
            "border border-neutral-200 bg-white text-neutral-700 hover:bg-neutral-50",
          ])}
        >
          Explore local AI setup
        </Link>
      </div>
    </section>
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
          <FAQItem question="Which AI models does Char use?">
            Char Cloud routes requests to the best models for each task.
          </FAQItem>
          <FAQItem question="Can I use different models for different meetings?">
            Yes. You can switch providers before any meeting or re-process
            existing transcripts with different models anytime.
          </FAQItem>
          <FAQItem question="What happens to my notes if I switch providers?">
            Nothing changes in your notes. They stay as Markdown files on your
            device.
          </FAQItem>
          <FAQItem question="Is local AI good enough?">
            Local models keep improving and work well for many meetings. Cloud
            models can still help for tougher reasoning-heavy conversations.
          </FAQItem>
          <FAQItem question="Does Char train AI models on my data?">
            No. Char does not use your recordings, transcripts, or notes to
            train AI models.
          </FAQItem>
        </FAQ>
      </div>
    </section>
  );
}
