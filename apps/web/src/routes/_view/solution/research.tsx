import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/solution/research")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Meeting Notes for Researchers - Char" },
      {
        name: "description",
        content:
          "Capture interviews, observations, and research insights with AI-powered meeting notes. Analyze faster with Char.",
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: "AI Meeting Notes for Researchers - Char",
      },
      {
        property: "og:description",
        content:
          "Focus on discovery while AI captures every detail. Turn research conversations into actionable insights.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/research",
      },
    ],
  }),
});

const features = [
  {
    icon: "mdi:microphone-variant",
    title: "Interview Recording",
    description:
      "Capture user interviews, expert conversations, and field observations. Get accurate transcriptions of every research session.",
  },
  {
    icon: "mdi:tag-multiple",
    title: "Theme Identification",
    description:
      "AI helps identify recurring themes, patterns, and insights across multiple research sessions.",
  },
  {
    icon: "mdi:format-quote-close",
    title: "Quote Extraction",
    description:
      "Easily find and extract participant quotes for reports, papers, and presentations.",
  },
  {
    icon: "mdi:file-search",
    title: "Search & Analysis",
    description:
      "Search across all your research notes to find specific topics, responses, or insights instantly.",
  },
  {
    icon: "mdi:book-open-variant",
    title: "Research Synthesis",
    description:
      "Turn raw interview data into structured findings. Support your analysis with detailed evidence.",
  },
  {
    icon: "mdi:shield-lock",
    title: "Participant Privacy",
    description:
      "Local AI processing ensures sensitive research data and participant information stays private.",
  },
];

const useCases = [
  {
    title: "User Research",
    description:
      "Capture user interviews, usability tests, and feedback sessions. Identify patterns in user needs and behaviors.",
  },
  {
    title: "Academic Research",
    description:
      "Record research interviews, focus groups, and field observations. Support qualitative analysis with detailed transcripts.",
  },
  {
    title: "Market Research",
    description:
      "Document customer conversations, market interviews, and competitive insights. Extract themes and trends.",
  },
  {
    title: "Ethnographic Studies",
    description:
      "Capture field notes, participant observations, and contextual inquiries. Build rich qualitative datasets.",
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
            <Icon icon="mdi:flask" className="text-lg" />
            <span>For Researchers</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Discover faster with
            <br />
            AI-powered meeting notes
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            Focus on asking questions and observing while Char captures every
            detail, identifies themes, and helps you analyze research
            conversations.
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
          Built for research excellence
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Every feature designed to help you capture, analyze, and synthesize
          research insights.
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
          For every research method
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Whatever your research approach, Char helps you capture and analyze
          every conversation.
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
          Ready to accelerate your research?
        </h2>
        <p className="mb-8 text-neutral-600">
          Join researchers who are discovering faster with AI-powered meeting
          notes.
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
