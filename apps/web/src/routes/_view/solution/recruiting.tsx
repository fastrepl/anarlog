import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/solution/recruiting")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Meeting Notes for Recruiting - Char" },
      {
        name: "description",
        content:
          "Capture every candidate interview with AI-powered meeting notes. Get structured feedback, compare candidates objectively, and make better hiring decisions with Char.",
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: "AI Meeting Notes for Recruiting - Char",
      },
      {
        property: "og:description",
        content:
          "Never miss candidate insights. AI-powered interview notes capture responses, assess skills, and help you make data-driven hiring decisions.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/recruiting",
      },
    ],
  }),
});

const features = [
  {
    icon: "mdi:account-search",
    title: "Capture Every Response",
    description:
      "Record candidate interviews and get complete transcriptions. Never miss important answers or red flags.",
  },
  {
    icon: "mdi:clipboard-check",
    title: "Structured Feedback",
    description:
      "AI extracts key competencies, skills mentioned, and cultural fit indicators from every interview.",
  },
  {
    icon: "mdi:scale-balance",
    title: "Objective Comparison",
    description:
      "Compare candidates fairly with consistent documentation. Reduce bias with comprehensive interview records.",
  },
  {
    icon: "mdi:share-variant",
    title: "Team Collaboration",
    description:
      "Share interview summaries with hiring managers and team members. Keep everyone aligned on candidate assessments.",
  },
  {
    icon: "mdi:clock-fast",
    title: "Faster Decisions",
    description:
      "Review interview highlights quickly. Make hiring decisions faster with AI-generated summaries and key points.",
  },
  {
    icon: "mdi:shield-lock",
    title: "Candidate Privacy",
    description:
      "Local AI processing keeps sensitive candidate information secure. Control what gets shared and stored.",
  },
];

const useCases = [
  {
    title: "Phone Screens",
    description:
      "Capture initial screening calls efficiently. Identify qualified candidates quickly with AI-extracted highlights.",
  },
  {
    title: "Technical Interviews",
    description:
      "Document technical discussions, coding explanations, and problem-solving approaches for thorough evaluation.",
  },
  {
    title: "Panel Interviews",
    description:
      "Record multi-interviewer sessions and consolidate feedback from all participants in one place.",
  },
  {
    title: "Hiring Committee Reviews",
    description:
      "Share comprehensive interview documentation with hiring committees for informed decision-making.",
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
            <Icon icon="mdi:account-tie" className="text-lg" />
            <span>For Recruiting Teams</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Hire better with
            <br />
            AI-powered interview notes
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            Focus on the candidate, not your notepad. Char captures every
            interview detail so you can make better hiring decisions.
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
          Built for recruiting excellence
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Every feature designed to help you find and hire the best talent.
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
          For every interview stage
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          From first screen to final round, capture what matters at every step.
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
          Ready to transform your hiring process?
        </h2>
        <p className="mb-8 text-neutral-600">
          Join recruiting teams who are making better hiring decisions with
          AI-powered interview notes.
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
