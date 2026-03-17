import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/solution/journalism")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Meeting Notes for Journalists - Char" },
      {
        name: "description",
        content:
          "Capture interviews, press briefings, and source conversations with AI-powered meeting notes. Report with confidence using Char.",
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: "AI Meeting Notes for Journalists - Char",
      },
      {
        property: "og:description",
        content:
          "Focus on the story while AI captures every quote. Get accurate transcriptions and never miss a detail.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/journalism",
      },
    ],
  }),
});

const features = [
  {
    icon: "mdi:microphone",
    title: "Interview Recording",
    description:
      "Record interviews, press conferences, and source conversations. Get accurate transcriptions with timestamps.",
  },
  {
    icon: "mdi:format-quote-open",
    title: "Precise Quotes",
    description:
      "Extract exact quotes with context. Ensure accuracy and attribution in your reporting.",
  },
  {
    icon: "mdi:magnify",
    title: "Fact Verification",
    description:
      "Search through interview transcripts to verify details, dates, and claims from your sources.",
  },
  {
    icon: "mdi:clock-fast",
    title: "Fast Turnaround",
    description:
      "Get instant transcriptions. Spend less time on notes and more time writing your story.",
  },
  {
    icon: "mdi:archive",
    title: "Source Documentation",
    description:
      "Maintain detailed records of all interviews and conversations. Protect your reporting with documentation.",
  },
  {
    icon: "mdi:shield-lock",
    title: "Source Protection",
    description:
      "Local AI processing ensures sensitive source information stays private and secure on your device.",
  },
];

const useCases = [
  {
    title: "Investigative Reporting",
    description:
      "Document complex interviews and source conversations. Build comprehensive records for long-form investigations.",
  },
  {
    title: "Breaking News",
    description:
      "Capture press briefings and rapid interviews. Get quotes fast for deadline reporting.",
  },
  {
    title: "Feature Stories",
    description:
      "Record in-depth interviews and personal narratives. Find the perfect quotes to bring your stories to life.",
  },
  {
    title: "Press Events",
    description:
      "Capture statements from press conferences, panel discussions, and media events with accurate attribution.",
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
        <FeaturesSection />
        <UseCasesSection />
        <CTASection />
      </div>
    </div>
  );
}

function HeroSection() {
  return (
    <div className="bg-linear-to-b from-stone-50/30 to-stone-100/30">
      <div className="px-6 py-12 lg:py-20">
        <header className="mx-auto mb-8 max-w-4xl text-center">
          <div className="mb-6 inline-flex items-center gap-2 rounded-full bg-stone-100 px-4 py-2 text-sm text-stone-600">
            <Icon icon="mdi:newspaper" className="text-lg" />
            <span>For Journalists</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Report with confidence using
            <br />
            AI-powered meeting notes
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            Focus on asking the right questions while Char captures every quote,
            verifies accuracy, and helps you tell compelling stories.
          </p>
          <div className="mt-8 flex flex-col justify-center gap-4 sm:flex-row">
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
            <Link
              to="/product/ai-notetaking/"
              className={cn([
                "inline-block rounded-full px-8 py-3 text-base font-medium",
                "border border-stone-300 text-stone-600",
                "transition-colors hover:bg-stone-50",
              ])}
            >
              See how it works
            </Link>
          </div>
        </header>
      </div>
    </div>
  );
}

function FeaturesSection() {
  return (
    <section className="border-t border-neutral-100 px-6 py-16">
      <div className="mx-auto max-w-4xl">
        <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
          Built for journalism excellence
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Every feature designed to help you report with accuracy and
          confidence.
        </p>
        <div className="grid gap-8 md:grid-cols-2 lg:grid-cols-3">
          {features.map((feature) => (
            <div key={feature.title} className="flex flex-col gap-3">
              <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-stone-100">
                <Icon icon={feature.icon} className="text-2xl text-stone-600" />
              </div>
              <h3 className="text-lg font-medium text-stone-700">
                {feature.title}
              </h3>
              <p className="text-sm leading-relaxed text-neutral-600">
                {feature.description}
              </p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function UseCasesSection() {
  return (
    <section className="border-t border-neutral-100 bg-stone-50/50 px-6 py-16">
      <div className="mx-auto max-w-4xl">
        <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
          For every beat and story
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Whatever you cover, Char helps you capture and verify every
          conversation.
        </p>
        <div className="grid gap-6 md:grid-cols-2">
          {useCases.map((useCase) => (
            <div
              key={useCase.title}
              className="rounded-xl border border-neutral-100 bg-white p-6"
            >
              <h3 className="mb-2 text-lg font-medium text-stone-700">
                {useCase.title}
              </h3>
              <p className="text-sm leading-relaxed text-neutral-600">
                {useCase.description}
              </p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function CTASection() {
  return (
    <section className="border-t border-neutral-100 px-6 py-16">
      <div className="mx-auto max-w-2xl text-center">
        <h2 className="mb-4 font-serif text-3xl text-stone-700">
          Ready to report with confidence?
        </h2>
        <p className="mb-8 text-neutral-600">
          Join journalists who are telling better stories with AI-powered
          meeting notes.
        </p>
        <Link
          to="/download/"
          className={cn([
            "inline-block rounded-full px-8 py-3 text-base font-medium",
            "bg-linear-to-t from-stone-600 to-stone-500 text-white",
            "transition-transform hover:scale-105 active:scale-95",
          ])}
        >
          Get started for free
        </Link>
      </div>
    </section>
  );
}
