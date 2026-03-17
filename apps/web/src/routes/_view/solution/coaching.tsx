import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/solution/coaching")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Meeting Notes for Coaches - Char" },
      {
        name: "description",
        content:
          "Capture coaching sessions, client progress, and breakthrough moments with AI-powered meeting notes. Focus on your clients with Char.",
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: "AI Meeting Notes for Coaches - Char",
      },
      {
        property: "og:description",
        content:
          "Be fully present with your clients while AI captures every insight. Track progress and deliver transformational coaching.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/coaching",
      },
    ],
  }),
});

const features = [
  {
    icon: "mdi:account-heart",
    title: "Full Presence",
    description:
      "Stop taking notes during sessions. Be fully present and engaged with your clients while AI captures everything.",
  },
  {
    icon: "mdi:chart-line",
    title: "Progress Tracking",
    description:
      "Track client goals, commitments, and breakthroughs across sessions. See patterns and progress over time.",
  },
  {
    icon: "mdi:lightbulb-on",
    title: "Insight Capture",
    description:
      "AI identifies key themes, mindset shifts, and breakthrough moments from coaching conversations.",
  },
  {
    icon: "mdi:clipboard-check",
    title: "Action Items",
    description:
      "Automatically extract commitments, homework, and action items from each session for follow-up.",
  },
  {
    icon: "mdi:message-reply-text",
    title: "Session Summaries",
    description:
      "Generate session summaries to share with clients. Help them remember key insights and next steps.",
  },
  {
    icon: "mdi:shield-lock",
    title: "Client Privacy",
    description:
      "Local AI processing ensures sensitive coaching conversations stay completely private and secure.",
  },
];

const useCases = [
  {
    title: "Life Coaching",
    description:
      "Capture personal goals, challenges, and transformation journeys. Track client progress toward their vision.",
  },
  {
    title: "Executive Coaching",
    description:
      "Document leadership challenges, strategic thinking, and professional development goals. Support high-stakes growth.",
  },
  {
    title: "Career Coaching",
    description:
      "Track career goals, skill development, and job search progress. Help clients navigate their career journey.",
  },
  {
    title: "Health & Wellness",
    description:
      "Record wellness goals, habit changes, and health milestones. Support clients in sustainable lifestyle changes.",
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
            <Icon icon="mdi:heart-pulse" className="text-lg" />
            <span>For Coaches</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Transform lives with
            <br />
            AI-powered meeting notes
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            Be fully present with your clients while Char captures every
            insight, tracks progress, and helps you deliver transformational
            coaching.
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
          Built for coaching excellence
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Every feature designed to help you be fully present and deliver
          transformational results.
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
          For every coaching practice
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Whatever your coaching specialty, Char helps you capture and leverage
          every client session.
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
          Ready to amplify your impact?
        </h2>
        <p className="mb-8 text-neutral-600">
          Join coaches who are transforming lives with AI-powered meeting notes.
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
