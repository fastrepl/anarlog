import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/solution/knowledge-workers")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Meeting Notes for Knowledge Workers - Char" },
      {
        name: "description",
        content:
          "Capture every meeting detail with AI-powered notes. Get automatic transcriptions, summaries, and action items. Focus on the conversation, not on taking notes.",
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: "AI Meeting Notes for Knowledge Workers - Char",
      },
      {
        property: "og:description",
        content:
          "Never miss important details. AI-powered meeting notes capture everything, extract action items, and help you stay organized.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/knowledge-workers",
      },
    ],
  }),
});

const features = [
  {
    icon: "mdi:microphone",
    title: "Capture Every Detail",
    description:
      "Record meetings automatically. Never miss important discussions, decisions, or action items.",
  },
  {
    icon: "mdi:text-box-check",
    title: "Smart Summaries",
    description:
      "AI extracts key points, decisions, and action items from every meeting automatically.",
  },
  {
    icon: "mdi:clipboard-list",
    title: "Action Items & Follow-ups",
    description:
      "Automatically identify next steps, commitments, and follow-up tasks from every conversation.",
  },
  {
    icon: "mdi:magnify",
    title: "Searchable Archive",
    description:
      "Find any meeting, note, or conversation instantly with powerful full-text search.",
  },
  {
    icon: "mdi:share-variant",
    title: "Easy Sharing",
    description:
      "Share meeting summaries with your team. Keep everyone aligned and informed.",
  },
  {
    icon: "mdi:shield-lock",
    title: "Privacy-First",
    description:
      "Your meetings stay private. Local AI processing means sensitive data never leaves your device.",
  },
];

const useCases = [
  {
    title: "Team Meetings",
    description:
      "Capture discussions, decisions, and action items. Keep your team aligned with shared meeting notes.",
  },
  {
    title: "Client Calls",
    description:
      "Focus on the conversation while AI captures requirements, feedback, and next steps.",
  },
  {
    title: "Brainstorming Sessions",
    description:
      "Record creative sessions and never lose a great idea. Review and organize thoughts later.",
  },
  {
    title: "One-on-Ones",
    description:
      "Document feedback, goals, and commitments. Build a searchable history of your conversations.",
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
          <div className="mb-6 inline-flex items-center gap-2 rounded-full bg-stone-100 px-4 py-2 text-sm text-stone-700">
            <Icon icon="mdi:account-group" className="text-lg" />
            <span>For Knowledge Workers</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Focus on the conversation,
            <br />
            not on taking notes
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            Stop scrambling to capture everything. Char records your meetings,
            transcribes conversations, and creates smart summaries so you can
            stay present.
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
                "border border-stone-300 text-stone-700",
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
          Built for how you work
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Every feature designed to help you capture, organize, and act on your
          meetings.
        </p>
        <div className="grid gap-8 md:grid-cols-2 lg:grid-cols-3">
          {features.map((feature) => (
            <div key={feature.title} className="flex flex-col gap-3">
              <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-stone-100">
                <Icon icon={feature.icon} className="text-2xl text-stone-700" />
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
          For every conversation
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          From team syncs to client calls, Char helps you capture and act on
          every interaction.
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
          Ready to transform your meetings?
        </h2>
        <p className="mb-8 text-neutral-600">
          Join knowledge workers who are getting more done with AI-powered
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
