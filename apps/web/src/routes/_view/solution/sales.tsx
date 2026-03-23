import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/solution/sales")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Meeting Notes for Sales Teams - Char" },
      {
        name: "description",
        content:
          "Capture every sales call detail with AI-powered meeting notes. Get automatic transcriptions, deal insights, and CRM-ready summaries. Close more deals with Char.",
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: "AI Meeting Notes for Sales Teams - Char",
      },
      {
        property: "og:description",
        content:
          "Never miss a sales opportunity. AI-powered meeting notes capture every detail, extract action items, and help you close deals faster.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/sales",
      },
    ],
  }),
});

const features = [
  {
    icon: "mdi:microphone",
    title: "Capture Every Detail",
    description:
      "Record sales calls and demos automatically. Never miss pricing discussions, objections, or buying signals.",
  },
  {
    icon: "mdi:text-box-check",
    title: "Deal Intelligence",
    description:
      "AI extracts key deal information, competitor mentions, budget discussions, and decision-maker insights.",
  },
  {
    icon: "mdi:clipboard-list",
    title: "Action Items & Follow-ups",
    description:
      "Automatically identify next steps, commitments, and follow-up tasks from every sales conversation.",
  },
  {
    icon: "mdi:chart-timeline-variant",
    title: "Sales Coaching Insights",
    description:
      "Review call recordings to improve pitch delivery, objection handling, and closing techniques.",
  },
  {
    icon: "mdi:share-variant",
    title: "Team Collaboration",
    description:
      "Share call summaries with your team. Keep everyone aligned on deal progress and customer needs.",
  },
  {
    icon: "mdi:shield-lock",
    title: "Privacy-First",
    description:
      "Your sales conversations stay private. Local AI processing means sensitive deal data never leaves your device.",
  },
];

const useCases = [
  {
    title: "Discovery Calls",
    description:
      "Capture prospect pain points, requirements, and buying criteria. Build comprehensive customer profiles from every conversation.",
  },
  {
    title: "Product Demos",
    description:
      "Focus on delivering great demos while AI captures questions, feature requests, and areas of interest.",
  },
  {
    title: "Negotiation Calls",
    description:
      "Track pricing discussions, contract terms, and stakeholder concerns. Never forget what was agreed upon.",
  },
  {
    title: "QBRs & Account Reviews",
    description:
      "Document customer feedback, renewal discussions, and expansion opportunities with detailed meeting notes.",
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
            <Icon icon="mdi:briefcase" className="text-lg" />
            <span>For Sales Teams</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Close more deals with
            <br />
            AI-powered meeting notes
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            Stop taking notes during sales calls. Focus on building
            relationships while Char captures every detail, extracts insights,
            and prepares your follow-ups.
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
          Built for sales professionals
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Every feature designed to help you sell more effectively and close
          deals faster.
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
          For every sales conversation
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          From first touch to closed-won, Char helps you capture and act on
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
          Ready to supercharge your sales?
        </h2>
        <p className="mb-8 text-neutral-600">
          Join sales teams who are closing more deals with AI-powered meeting
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
