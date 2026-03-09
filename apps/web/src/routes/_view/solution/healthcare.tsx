import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/solution/healthcare")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Meeting Notes for Healthcare - Char" },
      {
        name: "description",
        content:
          "HIPAA-ready AI meeting notes for healthcare professionals. Capture patient consultations, clinical meetings, and care coordination with privacy-first local AI processing.",
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: "AI Meeting Notes for Healthcare - Char",
      },
      {
        property: "og:description",
        content:
          "Privacy-first AI notetaking for healthcare. Local processing keeps patient data secure while capturing clinical discussions and care coordination meetings.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/healthcare",
      },
    ],
  }),
});

const features = [
  {
    icon: "mdi:shield-check",
    title: "Privacy-First Design",
    description:
      "Local AI processing keeps sensitive patient information on your device. No data leaves your system without your explicit consent.",
  },
  {
    icon: "mdi:microphone",
    title: "Clinical Documentation",
    description:
      "Capture patient consultations, care team meetings, and clinical discussions with accurate transcription.",
  },
  {
    icon: "mdi:clipboard-pulse",
    title: "Care Coordination",
    description:
      "Document handoffs, multidisciplinary team meetings, and care planning sessions with comprehensive notes.",
  },
  {
    icon: "mdi:account-group",
    title: "Team Collaboration",
    description:
      "Share meeting summaries with care teams while maintaining appropriate access controls and privacy.",
  },
  {
    icon: "mdi:clock-outline",
    title: "Time Savings",
    description:
      "Reduce documentation burden so clinicians can focus more time on patient care instead of paperwork.",
  },
  {
    icon: "mdi:server-security",
    title: "Self-Hosting Option",
    description:
      "Deploy on your own infrastructure for complete control over data residency and compliance requirements.",
  },
];

const useCases = [
  {
    title: "Patient Consultations",
    description:
      "Capture key discussion points, treatment plans, and follow-up actions from patient meetings without breaking eye contact.",
  },
  {
    title: "Care Team Meetings",
    description:
      "Document multidisciplinary rounds, case conferences, and care coordination discussions with all stakeholders.",
  },
  {
    title: "Administrative Meetings",
    description:
      "Track departmental meetings, quality improvement discussions, and operational planning sessions.",
  },
  {
    title: "Training & Education",
    description:
      "Record educational sessions, grand rounds, and training meetings for future reference and learning.",
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
            <Icon icon="mdi:hospital-building" className="text-lg" />
            <span>For Healthcare</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Privacy-first AI notes
            <br />
            for healthcare teams
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            Capture clinical meetings and patient discussions with AI that
            processes everything locally. Your patient data never leaves your
            device.
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
          Built for healthcare privacy
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Every feature designed with patient privacy and clinical workflows in
          mind.
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
          For every clinical conversation
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          From patient consultations to team meetings, capture what matters
          most.
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
          Ready to streamline clinical documentation?
        </h2>
        <p className="mb-8 text-neutral-600">
          Join healthcare teams who are saving time on documentation while
          maintaining patient privacy.
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
