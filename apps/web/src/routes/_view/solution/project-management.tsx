import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/solution/project-management")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Meeting Notes for Project Management - Char" },
      {
        name: "description",
        content:
          "Capture every project meeting with AI-powered notes. Track decisions, action items, and stakeholder discussions. Keep projects on track with Char.",
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: "AI Meeting Notes for Project Management - Char",
      },
      {
        property: "og:description",
        content:
          "Never miss project decisions. AI-powered meeting notes capture action items, stakeholder feedback, and keep your projects on track.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/project-management",
      },
    ],
  }),
});

const features = [
  {
    icon: "mdi:clipboard-check",
    title: "Action Item Tracking",
    description:
      "AI automatically extracts action items, owners, and deadlines from every project meeting.",
  },
  {
    icon: "mdi:account-group",
    title: "Stakeholder Management",
    description:
      "Document stakeholder meetings, capture feedback, and track decisions across all project phases.",
  },
  {
    icon: "mdi:chart-gantt",
    title: "Status Updates",
    description:
      "Generate meeting summaries for status reports. Keep stakeholders informed with accurate documentation.",
  },
  {
    icon: "mdi:alert-circle",
    title: "Risk Documentation",
    description:
      "Capture risk discussions, mitigation plans, and issue resolutions from project meetings.",
  },
  {
    icon: "mdi:share-variant",
    title: "Team Alignment",
    description:
      "Share meeting notes with project teams. Keep everyone aligned on decisions and next steps.",
  },
  {
    icon: "mdi:magnify",
    title: "Searchable History",
    description:
      "Find past decisions, commitments, and discussions across all your project meetings instantly.",
  },
];

const useCases = [
  {
    title: "Sprint Planning",
    description:
      "Capture sprint planning discussions, story refinements, and team commitments for agile projects.",
  },
  {
    title: "Stakeholder Reviews",
    description:
      "Document stakeholder feedback, approval decisions, and change requests from review meetings.",
  },
  {
    title: "Stand-ups & Check-ins",
    description:
      "Record daily stand-ups and team check-ins. Track blockers and progress across the project.",
  },
  {
    title: "Retrospectives",
    description:
      "Capture retrospective discussions, improvement ideas, and action items for continuous improvement.",
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
            <Icon icon="mdi:clipboard-text" className="text-lg" />
            <span>For Project Management</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Keep projects on track
            <br />
            with AI-powered notes
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            Focus on leading your projects, not taking notes. Char captures
            every meeting detail so nothing falls through the cracks.
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
          Built for project managers
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Every feature designed to help you deliver projects successfully.
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
          For every project meeting
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          From planning to retrospectives, capture what matters at every phase.
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
          Ready to improve project delivery?
        </h2>
        <p className="mb-8 text-neutral-600">
          Join project managers who are keeping projects on track with
          AI-powered meeting notes.
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
