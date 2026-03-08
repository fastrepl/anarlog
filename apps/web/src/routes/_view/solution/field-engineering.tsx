import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/solution/field-engineering")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Meeting Notes for Field Engineering - Char" },
      {
        name: "description",
        content:
          "Capture technical discussions and customer meetings on the go with AI-powered meeting notes. Document implementations, troubleshooting, and field visits with Char.",
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: "AI Meeting Notes for Field Engineering - Char",
      },
      {
        property: "og:description",
        content:
          "Never miss technical details. AI-powered meeting notes capture implementations, troubleshooting sessions, and customer discussions for field engineers.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/field-engineering",
      },
    ],
  }),
});

const features = [
  {
    icon: "mdi:wrench",
    title: "Technical Documentation",
    description:
      "Capture implementation details, configuration discussions, and technical decisions during customer visits.",
  },
  {
    icon: "mdi:bug",
    title: "Troubleshooting Records",
    description:
      "Document debugging sessions, root cause analyses, and resolution steps for future reference.",
  },
  {
    icon: "mdi:account-hard-hat",
    title: "On-Site Meetings",
    description:
      "Record customer meetings, site assessments, and technical reviews even in challenging environments.",
  },
  {
    icon: "mdi:share-variant",
    title: "Knowledge Sharing",
    description:
      "Share detailed technical notes with your team. Build a knowledge base from field experiences.",
  },
  {
    icon: "mdi:wifi-off",
    title: "Offline Capable",
    description:
      "Local AI processing works without internet. Capture meetings anywhere, sync when connected.",
  },
  {
    icon: "mdi:shield-lock",
    title: "Customer Privacy",
    description:
      "Local processing keeps sensitive customer infrastructure details secure on your device.",
  },
];

const useCases = [
  {
    title: "Implementation Meetings",
    description:
      "Document deployment discussions, configuration decisions, and integration requirements during customer implementations.",
  },
  {
    title: "Technical Support Calls",
    description:
      "Capture troubleshooting sessions, diagnostic findings, and resolution steps for complex technical issues.",
  },
  {
    title: "Site Assessments",
    description:
      "Record site survey findings, infrastructure evaluations, and technical recommendations for customers.",
  },
  {
    title: "Training Sessions",
    description:
      "Document customer training sessions, Q&A discussions, and follow-up action items.",
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
            <Icon icon="mdi:tools" className="text-lg" />
            <span>For Field Engineering</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Capture technical details
            <br />
            anywhere you work
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            From customer sites to remote locations, Char captures your
            technical discussions with AI that works offline.
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
              to="/product/local-ai/"
              className={cn([
                "inline-block rounded-full px-8 py-3 text-base font-medium",
                "border border-stone-300 text-stone-700",
                "transition-colors hover:bg-stone-50",
              ])}
            >
              Learn about local AI
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
          Built for field engineers
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Every feature designed for technical work in the field.
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
          For every field engagement
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          From implementations to support calls, capture what matters most.
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
          Ready to improve field documentation?
        </h2>
        <p className="mb-8 text-neutral-600">
          Join field engineers who are capturing every technical detail with
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
