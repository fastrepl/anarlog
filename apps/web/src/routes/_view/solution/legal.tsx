import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/solution/legal")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Meeting Notes for Legal Teams - Char" },
      {
        name: "description",
        content:
          "Confidential AI meeting notes for legal professionals. Capture client consultations, depositions, and case discussions with privacy-first local AI processing.",
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: "AI Meeting Notes for Legal Teams - Char",
      },
      {
        property: "og:description",
        content:
          "Attorney-client privilege protected. Local AI processing keeps confidential legal discussions secure while capturing every important detail.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/legal",
      },
    ],
  }),
});

const features = [
  {
    icon: "mdi:shield-lock",
    title: "Confidentiality First",
    description:
      "Local AI processing ensures attorney-client privileged communications never leave your device or network.",
  },
  {
    icon: "mdi:microphone",
    title: "Accurate Transcription",
    description:
      "Capture client meetings, depositions, and case discussions with precise transcription for your records.",
  },
  {
    icon: "mdi:file-document-outline",
    title: "Case Documentation",
    description:
      "Build comprehensive case files with detailed meeting notes, action items, and key discussion points.",
  },
  {
    icon: "mdi:clock-outline",
    title: "Billable Time Tracking",
    description:
      "Accurate meeting records help with time tracking and billing documentation for client matters.",
  },
  {
    icon: "mdi:magnify",
    title: "Searchable Archives",
    description:
      "Find specific discussions, decisions, or commitments across all your case-related meetings instantly.",
  },
  {
    icon: "mdi:server-security",
    title: "Self-Hosting Available",
    description:
      "Deploy on your firm's infrastructure for complete control over data residency and compliance.",
  },
];

const useCases = [
  {
    title: "Client Consultations",
    description:
      "Capture initial consultations and ongoing client meetings with complete confidentiality and accuracy.",
  },
  {
    title: "Case Strategy Sessions",
    description:
      "Document internal case discussions, strategy planning, and team deliberations for comprehensive records.",
  },
  {
    title: "Witness Interviews",
    description:
      "Record and transcribe witness interviews with accurate documentation for case preparation.",
  },
  {
    title: "Partner & Team Meetings",
    description:
      "Track firm meetings, case assignments, and administrative discussions with detailed notes.",
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
            <Icon icon="mdi:scale-balance" className="text-lg" />
            <span>For Legal Teams</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Confidential AI notes
            <br />
            for legal professionals
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            Capture every client meeting and case discussion with AI that
            processes everything locally. Your privileged communications stay
            protected.
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
                "border border-stone-300 text-stone-600",
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
          Built for legal confidentiality
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Every feature designed with attorney-client privilege and legal
          workflows in mind.
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
          For every legal conversation
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          From client consultations to case strategy, capture what matters most.
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
          Ready to streamline legal documentation?
        </h2>
        <p className="mb-8 text-neutral-600">
          Join legal teams who are saving time on documentation while
          maintaining client confidentiality.
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
