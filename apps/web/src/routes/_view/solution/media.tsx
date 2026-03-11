import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/solution/media")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Meeting Notes for Media & Entertainment - Char" },
      {
        name: "description",
        content:
          "Capture creative meetings, production calls, and editorial discussions with AI-powered meeting notes. Streamline content production workflows with Char.",
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: "AI Meeting Notes for Media & Entertainment - Char",
      },
      {
        property: "og:description",
        content:
          "Never miss creative ideas. AI-powered meeting notes capture brainstorms, production discussions, and editorial meetings for media teams.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/media",
      },
    ],
  }),
});

const features = [
  {
    icon: "mdi:lightbulb-on",
    title: "Capture Creative Ideas",
    description:
      "Record brainstorming sessions and creative meetings. Never lose brilliant ideas that come up in the moment.",
  },
  {
    icon: "mdi:movie-open",
    title: "Production Coordination",
    description:
      "Document production meetings, scheduling discussions, and logistics planning with comprehensive notes.",
  },
  {
    icon: "mdi:pencil",
    title: "Editorial Workflows",
    description:
      "Track editorial meetings, content reviews, and approval discussions with accurate transcription.",
  },
  {
    icon: "mdi:account-group",
    title: "Team Collaboration",
    description:
      "Share meeting summaries with creative teams, producers, and stakeholders to keep everyone aligned.",
  },
  {
    icon: "mdi:magnify",
    title: "Searchable Archives",
    description:
      "Find specific creative discussions, decisions, or feedback across all your project meetings instantly.",
  },
  {
    icon: "mdi:shield-lock",
    title: "Protect Creative IP",
    description:
      "Local AI processing keeps sensitive creative content and unreleased projects secure on your device.",
  },
];

const useCases = [
  {
    title: "Creative Brainstorms",
    description:
      "Capture every idea from creative sessions. Review and develop concepts that emerged during collaborative discussions.",
  },
  {
    title: "Production Meetings",
    description:
      "Document production schedules, resource allocation, and logistics discussions for smooth project execution.",
  },
  {
    title: "Client Reviews",
    description:
      "Record client feedback sessions and approval meetings. Track revisions and sign-offs accurately.",
  },
  {
    title: "Post-Production",
    description:
      "Capture editing notes, color correction discussions, and final review feedback for quality delivery.",
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
            <Icon icon="mdi:movie" className="text-lg" />
            <span>For Media & Entertainment</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Capture every creative idea
            <br />
            with AI-powered notes
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            From brainstorms to production calls, Char captures your creative
            discussions so no brilliant idea gets lost.
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
          Built for creative teams
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Every feature designed to support creative workflows and content
          production.
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
          For every production phase
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          From concept to delivery, capture what matters at every stage.
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
          Ready to streamline your creative workflow?
        </h2>
        <p className="mb-8 text-neutral-600">
          Join media teams who are capturing every creative idea with AI-powered
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
