import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/solution/government")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Meeting Notes for Government - Char" },
      {
        name: "description",
        content:
          "Secure AI meeting notes for government agencies. Capture public meetings, inter-agency coordination, and policy discussions with privacy-first local AI processing.",
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: "AI Meeting Notes for Government - Char",
      },
      {
        property: "og:description",
        content:
          "Secure, compliant AI notetaking for government. Local processing keeps sensitive discussions protected while improving meeting documentation.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/government",
      },
    ],
  }),
});

const features = [
  {
    icon: "mdi:shield-check",
    title: "Security First",
    description:
      "Local AI processing keeps sensitive government discussions on your secure infrastructure. No data leaves your network.",
  },
  {
    icon: "mdi:file-document-check",
    title: "Compliance Ready",
    description:
      "Self-hosting options support FedRAMP, FISMA, and other government compliance requirements.",
  },
  {
    icon: "mdi:account-multiple",
    title: "Public Meeting Records",
    description:
      "Create accurate records of public meetings, hearings, and community sessions for transparency.",
  },
  {
    icon: "mdi:handshake",
    title: "Inter-Agency Coordination",
    description:
      "Document cross-agency meetings, task force discussions, and collaborative initiatives.",
  },
  {
    icon: "mdi:magnify",
    title: "Searchable Archives",
    description:
      "Find specific discussions, decisions, or action items across all your meeting records instantly.",
  },
  {
    icon: "mdi:server-security",
    title: "Self-Hosting Available",
    description:
      "Deploy on government infrastructure for complete control over data residency and security.",
  },
];

const useCases = [
  {
    title: "Public Meetings",
    description:
      "Create accurate transcripts of town halls, council meetings, and public hearings for transparency and record-keeping.",
  },
  {
    title: "Policy Discussions",
    description:
      "Document internal policy meetings, strategy sessions, and planning discussions with comprehensive notes.",
  },
  {
    title: "Inter-Agency Coordination",
    description:
      "Capture cross-agency meetings, joint task forces, and collaborative initiatives with all stakeholders.",
  },
  {
    title: "Constituent Services",
    description:
      "Record constituent meetings and service discussions to ensure follow-through and accountability.",
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
            <Icon icon="mdi:bank" className="text-lg" />
            <span>For Government</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Secure AI notes for
            <br />
            government agencies
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            Capture public meetings and policy discussions with AI that
            processes everything locally. Your sensitive data stays on your
            secure infrastructure.
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
              to="/product/self-hosting/"
              className={cn([
                "inline-block rounded-full px-8 py-3 text-base font-medium",
                "border border-stone-300 text-stone-600",
                "transition-colors hover:bg-stone-50",
              ])}
            >
              Learn about self-hosting
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
          Built for government security
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Every feature designed with government security and compliance
          requirements in mind.
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
          For every government meeting
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          From public hearings to internal planning, capture what matters most.
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
          Ready to improve government documentation?
        </h2>
        <p className="mb-8 text-neutral-600">
          Join government agencies who are improving meeting documentation while
          maintaining security.
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
