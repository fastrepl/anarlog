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
    <main className="min-h-screen flex-1">
      <div className="mx-auto">
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
    <div className="border-b border-neutral-100 text-left">
      <p className="text-color-muted py-6 font-mono font-medium tracking-wide uppercase">
        {children}
      </p>
    </div>
  );
}

function HeroSection() {
  return (
    <section className="bg-linear-to-b from-stone-50/30 to-stone-100/30">
      <div className="flex flex-col items-center gap-6 px-4 py-24 text-left">
        <div className="flex max-w-4xl flex-col gap-6">
          <h1 className="text-color font-mono text-4xl tracking-tight sm:text-5xl">
            Take Meeting Notes With
            <br />
            AI of Your Choice
          </h1>
          <p className="text-color-muted mx-auto max-w-3xl text-lg sm:text-xl">
            Char lets you choose between managed cloud AI, your own provider
            keys, or fully local models on your machine.
          </p>
        </div>
        <div className="flex flex-col items-center gap-4 pt-6 sm:flex-row">
          <DownloadButton />
          <GithubStars />
        </div>
      </div>
    </section>
  );
}

function AISetupSection() {
  return (
    <section>
      <SectionTitle>Pick your AI setup</SectionTitle>
      <div className="grid md:grid-cols-3">
        {setupOptions.map((option, index) => (
          <div
            key={option.title}
            className={cn([
              "border-b border-neutral-100 p-8",
              index < setupOptions.length - 1 && "md:border-r",
            ])}
          >
            <Icon icon={option.icon} className="text-color mb-4 text-3xl" />
            <p className="text-color-secondary mb-1 font-mono text-sm">
              {option.eyebrow}
            </p>
            <h3 className="text-color mb-2 font-mono text-xl">
              {option.title}
            </h3>
            <p className="text-color mb-4 text-sm font-medium">
              {option.detail}
            </p>
            <p className="text-color-muted">{option.description}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

function LocalFeaturesSection() {
  return (
    <section>
      <SectionTitle>Local features</SectionTitle>
      <div className="divide-y divide-neutral-100">
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
              <p className="text-color-muted">{capability.description}</p>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function SwitchSection() {
  return (
    <section>
      <SectionTitle>Switch providers anytime</SectionTitle>
      <p className="text-color-muted border-b border-neutral-100 px-4 py-6 text-left">
        Your notes are never locked to a single AI provider.
      </p>
      <div className="grid md:grid-cols-2">
        {switchBenefits.map((benefit, index) => (
          <div
            key={benefit.title}
            className={cn([
              "border-neutral-100 p-8",
              index < 2 && "border-b",
              index % 2 === 0 && "md:border-r",
            ])}
          >
            <h3 className="text-color mb-2 font-mono text-lg">
              {benefit.title}
            </h3>
            <p className="text-color-muted">{benefit.description}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

function BenchmarkSection() {
  return (
    <section className="bg-linear-to-b from-stone-50/30 to-stone-100/30">
      <div className="flex flex-col items-center gap-6 px-4 py-16 text-left">
        <h2 className="text-color font-mono text-2xl sm:text-3xl">
          Compare model performance before you decide
        </h2>
        <p className="text-color-muted mx-auto max-w-2xl">
          We benchmark leading models on meeting tasks like summaries, action
          items, speaker tracking, and Q&A so you can choose with confidence.
        </p>
        <div className="flex flex-col gap-4 sm:flex-row">
          <Link
            to="/eval/"
            className={cn([
              "rounded-full px-8 py-3 text-base font-medium",
              "border border-neutral-300 text-color",
              "transition-colors hover:bg-stone-50",
            ])}
          >
            View AI model evaluations
          </Link>
          <Link
            to="/product/local-ai/"
            className={cn([
              "rounded-full px-8 py-3 text-base font-medium",
              "border border-neutral-300 text-color",
              "transition-colors hover:bg-stone-50",
            ])}
          >
            Explore local AI setup
          </Link>
        </div>
      </div>
    </section>
  );
}

function FAQSection() {
  return (
    <section className="px-4 py-16">
      <div className="mx-auto max-w-4xl">
        <div className="mb-12 text-left">
          <h2 className="text-color font-mono text-3xl">
            Frequently asked questions
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
