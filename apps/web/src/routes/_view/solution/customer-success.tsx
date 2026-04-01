import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/solution/customer-success")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Meeting Notes for Customer Success - Char" },
      {
        name: "description",
        content:
          "Capture every customer conversation with AI-powered meeting notes. Track health signals, document feedback, and drive retention with Char.",
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: "AI Meeting Notes for Customer Success - Char",
      },
      {
        property: "og:description",
        content:
          "Never miss customer insights. AI-powered meeting notes capture feedback, track health signals, and help you drive customer retention.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/customer-success",
      },
    ],
  }),
});

const features = [
  {
    icon: "mdi:heart-pulse",
    title: "Track Health Signals",
    description:
      "AI identifies customer sentiment, concerns, and satisfaction indicators from every conversation.",
  },
  {
    icon: "mdi:message-text",
    title: "Capture Feedback",
    description:
      "Document feature requests, pain points, and product feedback to share with your product team.",
  },
  {
    icon: "mdi:clipboard-check",
    title: "Action Items",
    description:
      "Automatically extract commitments, follow-ups, and action items from customer meetings.",
  },
  {
    icon: "mdi:chart-line",
    title: "Renewal Preparation",
    description:
      "Build comprehensive account histories to prepare for renewal conversations and expansion opportunities.",
  },
  {
    icon: "mdi:share-variant",
    title: "Team Handoffs",
    description:
      "Share detailed meeting notes for smooth handoffs between CSMs and across account teams.",
  },
  {
    icon: "mdi:shield-lock",
    title: "Customer Privacy",
    description:
      "Local AI processing keeps sensitive customer conversations secure. Control what gets shared and stored.",
  },
];

const useCases = [
  {
    title: "Onboarding Calls",
    description:
      "Document onboarding sessions, training discussions, and implementation meetings for successful customer launches.",
  },
  {
    title: "QBRs & Business Reviews",
    description:
      "Capture quarterly business reviews, success metrics discussions, and strategic planning sessions.",
  },
  {
    title: "Support Escalations",
    description:
      "Record escalation calls and resolution discussions. Build comprehensive case histories for complex issues.",
  },
  {
    title: "Renewal Conversations",
    description:
      "Document renewal discussions, contract negotiations, and expansion conversations with complete accuracy.",
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
            <Icon icon="mdi:account-heart" className="text-lg" />
            <span>For Customer Success</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Drive retention with
            <br />
            AI-powered meeting notes
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            Focus on your customers, not your notepad. Char captures every
            conversation detail so you can drive success and retention.
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
          Built for customer success
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Every feature designed to help you understand and retain your
          customers.
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
          For every customer interaction
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          From onboarding to renewal, capture what matters at every touchpoint.
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
          Ready to improve customer retention?
        </h2>
        <p className="mb-8 text-neutral-600">
          Join customer success teams who are driving retention with AI-powered
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
