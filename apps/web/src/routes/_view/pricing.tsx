import { createFileRoute, Link } from "@tanstack/react-router";
import { CheckCircle2, Construction, XCircle } from "lucide-react";

import { cn } from "@hypr/utils";

import { Image } from "@/components/image";
import { SlashSeparator } from "@/components/slash-separator";

export const Route = createFileRoute("/_view/pricing")({
  component: Component,
});

interface PricingPlan {
  name: string;
  price: { monthly: number; yearly: number | null } | null;
  description: string;
  popular?: boolean;
  features: Array<{
    label: string;
    included: boolean | "partial";
    tooltip?: string;
  }>;
}

const pricingPlans: PricingPlan[] = [
  {
    name: "Free",
    price: null,
    description:
      "Fully functional with your own API keys. Perfect for individuals who want complete control.",
    features: [
      { label: "On-device Transcription", included: true },
      { label: "Save Audio Recordings", included: true },
      { label: "Audio Player", included: true },
      { label: "Bring Your Own Key", included: true },
      { label: "Export to Various Formats", included: true },
      {
        label: "Custom Default Folder",
        included: true,
        tooltip: "Move your default folder location to anywhere you prefer.",
      },
      { label: "Chat", included: true },
      { label: "Contacts View", included: true },
      { label: "Calendar View", included: true },
      { label: "Transcript Editor", included: "partial" },
      { label: "Templates", included: "partial" },
      { label: "Shortcuts", included: "partial" },
      { label: "Cloud Services (STT & LLM)", included: false },
      { label: "Speaker Identification", included: false },
    ],
  },
  {
    name: "Lite",
    price: {
      monthly: 8,
      yearly: null,
    },
    description:
      "Unlimited cloud transcription and AI models without the complexity. No API keys needed — just sign in and go.",
    features: [
      { label: "Everything in Free", included: true },
      { label: "Cloud Services (STT & LLM)", included: true },
      { label: "Speaker Identification", included: "partial" },
      { label: "Change Playback Rates", included: false },
      { label: "Integrations", included: false },
      { label: "Advanced Templates", included: false },
      { label: "Folders View", included: false },
      { label: "Cloud Sync", included: false },
      { label: "Shareable Links", included: false },
    ],
  },
  {
    name: "Pro",
    price: {
      monthly: 25,
      yearly: 250,
    },
    description:
      "Everything in Lite, plus advanced sharing and team features out of the box.",
    popular: true,
    features: [
      { label: "Everything in Lite", included: true },
      { label: "Change Playback Rates", included: true },
      {
        label: "Integrations",
        included: true,
        tooltip:
          "Google Calendar is available now. Additional integrations are in progress.",
      },
      { label: "Advanced Templates", included: "partial" },
      { label: "Folders View", included: "partial" },
      {
        label: "Connect to OpenClaw",
        included: "partial",
        tooltip: "Select which notes to sync",
      },
      {
        label: "Cloud Sync",
        included: "partial",
        tooltip: "Select which notes to sync",
      },
      {
        label: "Shareable Links",
        included: "partial",
        tooltip: "DocSend-like: view tracking, expiration, revocation",
      },
    ],
  },
];

function Component() {
  return (
    <main
      className="min-h-screen flex-1 bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <HeroSection />
        <SlashSeparator />
        <PricingCardsSection />
        <SlashSeparator />
        <FAQSection />
        <SlashSeparator />
        <CTASection />
      </div>
    </main>
  );
}

function HeroSection() {
  return (
    <section className="laptop:px-0 flex flex-col items-center gap-6 border-b border-neutral-100 px-4 py-24 text-center">
      <div className="flex max-w-3xl flex-col gap-4">
        <h1 className="font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
          Pricing
        </h1>
        <p className="text-lg text-neutral-600 sm:text-xl">
          Start for free, upgrade when you need cloud features.
        </p>
      </div>
    </section>
  );
}

function PricingCardsSection() {
  return (
    <section className="laptop:px-0 px-4 py-16">
      <div className="mx-auto grid max-w-5xl grid-cols-1 items-stretch gap-8 md:grid-cols-3">
        {pricingPlans.map((plan) => (
          <PricingCard key={plan.name} plan={plan} />
        ))}
      </div>
    </section>
  );
}

function PricingCard({ plan }: { plan: PricingPlan }) {
  return (
    <div
      className={cn([
        "flex flex-col overflow-hidden rounded-xs border transition-transform",
        plan.popular
          ? "relative border-stone-600 shadow-lg"
          : "border-neutral-100",
      ])}
    >
      {plan.popular ? (
        <div className="bg-stone-600 px-4 py-2 text-center text-sm font-medium text-white">
          Most Popular
        </div>
      ) : (
        <div className="px-4 py-2 text-sm">&nbsp;</div>
      )}

      <div className="flex flex-1 flex-col p-8">
        <div className="mb-6">
          <h2 className="mb-2 font-serif text-2xl text-stone-700">
            {plan.name}
          </h2>
          <p className="mb-4 min-h-[80px] text-sm text-neutral-600">
            {plan.description}
          </p>

          <div className="min-h-[64px]">
            {plan.price ? (
              <div className="flex flex-col gap-2">
                <div className="flex items-baseline gap-2">
                  <span className="font-serif text-4xl text-stone-700">
                    ${plan.price.monthly}
                  </span>
                  <span className="text-neutral-600">/month</span>
                  {plan.price.yearly != null ? (
                    <span className="text-sm text-neutral-600">
                      or ${plan.price.yearly}/year
                    </span>
                  ) : null}
                </div>
              </div>
            ) : (
              <div className="flex items-baseline gap-2">
                <span className="font-serif text-4xl text-stone-700">$0</span>
                <span className="text-neutral-600">per month</span>
              </div>
            )}
          </div>
        </div>

        <div className="flex flex-col gap-3">
          {plan.features.map((feature, idx) => {
            const IconComponent =
              feature.included === true
                ? CheckCircle2
                : feature.included === "partial"
                  ? Construction
                  : XCircle;
            const hoverTitle =
              feature.included === "partial"
                ? "Currently in development"
                : undefined;

            return (
              <div
                key={idx}
                className="flex items-start gap-3"
                title={hoverTitle}
              >
                <IconComponent
                  className={cn([
                    "mt-0.5 size-4.5 shrink-0",
                    feature.included === true
                      ? "text-green-700"
                      : feature.included === "partial"
                        ? "text-yellow-600"
                        : "text-red-500",
                  ])}
                />
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <span
                      className={cn([
                        "text-sm",
                        feature.included === false
                          ? "text-neutral-700"
                          : "text-neutral-900",
                      ])}
                    >
                      {feature.label}
                    </span>
                  </div>
                  {feature.tooltip && (
                    <div className="mt-0.5 text-xs text-neutral-500 italic">
                      {feature.tooltip}
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>

        <div className="mt-auto pt-8">
          {plan.price ? (
            <Link
              to="/app/checkout/"
              search={{
                plan: plan.name.toLowerCase() as "lite" | "pro",
                period: "monthly",
              }}
              className={cn([
                "flex h-10 w-full cursor-pointer items-center justify-center text-sm font-medium transition-all",
                plan.popular
                  ? "rounded-full bg-linear-to-t from-stone-600 to-stone-500 text-white shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]"
                  : "rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 shadow-xs hover:scale-[102%] hover:shadow-md active:scale-[98%]",
              ])}
            >
              Get Started
            </Link>
          ) : (
            <Link
              to="/download/"
              className={cn([
                "flex h-10 w-full cursor-pointer items-center justify-center text-sm font-medium transition-all",
                "rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 shadow-xs hover:scale-[102%] hover:shadow-md active:scale-[98%]",
              ])}
            >
              Download for free
            </Link>
          )}
        </div>
      </div>
    </div>
  );
}

function FAQSection() {
  const faqs = [
    {
      question: "What does on-device transcription mean?",
      answer:
        "All transcription happens on your device. Your audio never leaves your computer, ensuring complete privacy.",
    },
    {
      question: "What is local-first data architecture?",
      answer:
        "Your data is filesystem-based by default: notes and transcripts are saved on your device first, and you stay in control of where files live.",
    },
    {
      question: "What is BYOK (Bring Your Own Key)?",
      answer:
        "BYOK allows you to connect your own LLM provider (like OpenAI, Anthropic, or self-hosted models) for AI features while maintaining full control over your data.",
    },
    {
      question: "What value does an account unlock?",
      answer:
        "A paid plan unlocks Char's cloud layer. Lite gives you hosted transcription, speaker identification, and language models, while Pro adds advanced templates, integrations, sync across devices, and shareable links.",
    },
    {
      question: "What's included in shareable links?",
      answer:
        "Pro users get DocSend-like controls: track who views your notes, set expiration dates, and revoke access anytime.",
    },
    {
      question: "What are templates?",
      answer:
        "Templates are our opinionated way to structure summaries. You can pick from a variety of templates we provide and create your own version as needed.",
    },
    {
      question: "What are advanced templates?",
      answer:
        "Advanced templates let you override Char’s default system prompt by configuring template variables and the overall instructions given to the AI.",
    },
    {
      question: "What are shortcuts?",
      answer:
        "Shortcuts are saved prompts you use repeatedly, like “Write a follow-up to blog blah” or “Create a one-pager of the important stuff that’s been discussed.” They’re available in chat via the / command.",
    },
    {
      question: "Do you offer student discounts?",
      answer:
        "Yes, we provide student discounts. Contact us and we’ll help you get set up with student pricing.",
    },
  ];

  return (
    <section className="laptop:px-0 border-t border-neutral-100 px-4 py-16">
      <div className="mx-auto max-w-3xl">
        <h2 className="mb-16 text-center font-serif text-3xl text-stone-700">
          Frequently Asked Questions
        </h2>
        <div className="flex flex-col gap-6">
          {faqs.map((faq, idx) => (
            <div
              key={idx}
              className="border-b border-neutral-100 pb-6 last:border-b-0"
            >
              <h3 className="mb-2 text-lg font-medium text-neutral-900">
                {faq.question}
              </h3>
              <p className="text-neutral-600">{faq.answer}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function CTASection() {
  return (
    <section className="laptop:px-0 border-t border-neutral-100 bg-linear-to-t from-stone-50/30 to-stone-100/30 px-4 py-16">
      <div className="flex flex-col items-center gap-6 text-center">
        <div className="mb-4 flex size-40 items-center justify-center rounded-[48px] border border-neutral-100 bg-transparent shadow-2xl">
          <Image
            src="/api/images/hyprnote/icon.png"
            alt="Char"
            width={144}
            height={144}
            className="mx-auto size-36 rounded-[40px] border border-neutral-100"
          />
        </div>
        <h2 className="font-serif text-2xl sm:text-3xl">Need a team plan?</h2>
        <p className="mx-auto max-w-2xl text-lg text-neutral-600">
          Book a call to discuss custom team pricing and enterprise solutions
        </p>
        <div className="pt-6">
          <Link
            to="/founders/"
            search={{ source: "team-plan" }}
            className="flex h-12 items-center justify-center rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-6 text-base text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%] sm:text-lg"
          >
            Book a call
          </Link>
        </div>
      </div>
    </section>
  );
}
