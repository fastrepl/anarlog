import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/solution/consulting")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Meeting Notes for Consultants - Char" },
      {
        name: "description",
        content:
          "Capture client conversations, project details, and strategic insights with AI-powered meeting notes. Deliver better recommendations with Char.",
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: "AI Meeting Notes for Consultants - Char",
      },
      {
        property: "og:description",
        content:
          "Focus on advising clients while AI captures every detail. Turn meeting notes into actionable insights and deliverables.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/consulting",
      },
    ],
  }),
});

const features = [
  {
    icon: "mdi:briefcase-check",
    title: "Client Meeting Capture",
    description:
      "Record client discussions, requirements, and strategic priorities. Keep detailed records of every engagement.",
  },
  {
    icon: "mdi:lightbulb",
    title: "Insight Extraction",
    description:
      "AI identifies key themes, pain points, and opportunities from client conversations to inform your recommendations.",
  },
  {
    icon: "mdi:file-document-multiple",
    title: "Deliverable Support",
    description:
      "Turn meeting notes into reports, presentations, and recommendations. Extract quotes and data points for your deliverables.",
  },
  {
    icon: "mdi:account-group",
    title: "Stakeholder Management",
    description:
      "Track different stakeholder perspectives, concerns, and priorities across multiple conversations.",
  },
  {
    icon: "mdi:clock-check",
    title: "Billable Hours",
    description:
      "Accurate meeting records support time tracking and billing documentation for client engagements.",
  },
  {
    icon: "mdi:shield-lock",
    title: "Client Confidentiality",
    description:
      "Local AI processing ensures sensitive client information stays private and secure on your device.",
  },
];

const useCases = [
  {
    title: "Discovery & Scoping",
    description:
      "Capture project requirements, success criteria, and constraints. Build comprehensive understanding of client needs.",
  },
  {
    title: "Strategy Sessions",
    description:
      "Document strategic discussions, decision rationale, and alignment points. Create clear records of recommendations.",
  },
  {
    title: "Stakeholder Interviews",
    description:
      "Record perspectives from different stakeholders. Identify patterns and conflicts across the organization.",
  },
  {
    title: "Status Updates & Reviews",
    description:
      "Track project progress, client feedback, and change requests. Maintain detailed engagement history.",
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
            <Icon icon="mdi:briefcase-account" className="text-lg" />
            <span>For Consultants</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Deliver better insights with
            <br />
            AI-powered meeting notes
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            Focus on advising your clients while Char captures every detail,
            extracts insights, and helps you create compelling deliverables.
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
          Built for consulting excellence
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Every feature designed to help you deliver exceptional client value
          and insights.
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
          For every client engagement
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          From discovery to delivery, Char helps you capture and leverage every
          client interaction.
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
          Ready to elevate your consulting?
        </h2>
        <p className="mb-8 text-neutral-600">
          Join consultants who are delivering better insights with AI-powered
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
