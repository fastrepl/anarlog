import { createFileRoute, Link } from "@tanstack/react-router";
import { CheckCircle2, XCircle } from "lucide-react";
import { motion } from "motion/react";

import { cn } from "@hypr/utils";

import { Image } from "@/components/image";

export const Route = createFileRoute("/_view/pricing")({
  component: Component,
});

interface PricingPlan {
  name: string;
  price: { monthly: number; yearly?: number };
  description: string;
  popular?: boolean;
  cta: { label: string; href: string };
  features: Array<{
    label: string;
    included: boolean;
    tooltip?: string;
  }>;
}

const pricingPlans: PricingPlan[] = [
  {
    name: "Free",
    price: { monthly: 0 },
    description:
      "Fully functional with your own API keys. Perfect for individuals who want complete control.",
    cta: { label: "Download for free", href: "/download/" },
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
      { label: "Transcript Editor", included: true },
      { label: "Templates", included: true },
      { label: "Shortcuts", included: true },
      { label: "Cloud Services (STT & LLM)", included: false },
      { label: "Speaker Identification", included: false },
    ],
  },
  {
    name: "Lite",
    price: { monthly: 8 },
    description:
      "Unlimited cloud transcription and AI models without the complexity. No API keys needed — just sign in and go.",
    cta: {
      label: "Get Started",
      href: "/app/checkout/?plan=lite&period=monthly",
    },
    features: [
      { label: "Everything in Free", included: true },
      { label: "Cloud Services (STT & LLM)", included: true },
      { label: "Speaker Identification", included: true },
      { label: "Change Playback Rates", included: true },
      { label: "Integrations", included: false },
      { label: "Advanced Templates", included: false },
      { label: "Folders View", included: false },
      { label: "Cloud Sync", included: false },
      { label: "Shareable Links", included: false },
    ],
  },
  {
    name: "Pro",
    price: { monthly: 25, yearly: 250 },
    description:
      "Everything in Lite, plus advanced sharing and team features out of the box.",
    popular: true,
    cta: {
      label: "Get Started",
      href: "/app/checkout/?plan=pro&period=monthly",
    },
    features: [
      { label: "Everything in Lite", included: true },
      { label: "Change Playback Rates", included: true },
      {
        label: "Integrations",
        included: true,
        tooltip:
          "Google Calendar is available now. Additional integrations are in progress.",
      },
      { label: "Advanced Templates", included: true },
      { label: "Folders View", included: true },
      {
        label: "Connect to OpenClaw",
        included: true,
        tooltip: "Select which notes to sync",
      },
      {
        label: "Cloud Sync",
        included: true,
        tooltip: "Select which notes to sync",
      },
      {
        label: "Shareable Links",
        included: true,
        tooltip: "DocSend-like: view tracking, expiration, revocation",
      },
    ],
  },
];

function Component() {
  return (
    <main className="laptop:px-6 min-h-screen flex-1 px-4">
      <div className="mx-auto">
        <HeroSection />
        <PricingCardsSection />
        <FAQSection />
        <CTASection />
      </div>
    </main>
  );
}

function HeroSection() {
  return (
    <section className="border-color-bright flex flex-col gap-6 border-b pt-16 pb-16 text-left md:pt-24">
      <div className="flex max-w-3xl flex-col gap-4">
        <h1 className="text-fg font-mono text-4xl tracking-tight sm:text-5xl">
          Pricing
        </h1>
        <p className="text-fg text-lg sm:text-xl">
          Start for free, upgrade when you need cloud features.
        </p>
      </div>
    </section>
  );
}

function PricingCardsSection() {
  return (
    <section className="py-16">
      <div className="mx-auto grid grid-cols-1 gap-4 md:grid-cols-3">
        {pricingPlans.map((plan) => (
          <PricingCard key={plan.name} plan={plan} />
        ))}
      </div>
    </section>
  );
}

function PricingCard({ plan }: { plan: PricingPlan }) {
  const isPaid = plan.price.monthly > 0;

  return (
    <motion.div
      whileHover={{ scale: 1.02, shadow: "0 0 10px 1 rgba(0, 0, 4, 0.35)" }}
      transition={{ type: "easeInOut" }}
      className={cn([
        "flex flex-col overflow-hidden rounded-xl border",
        plan.popular
          ? "border-color-bright surface relative shadow-lg"
          : "border-color-bright surface",
      ])}
    >
      <div className="flex flex-1 flex-col p-8">
        <div className="mb-6">
          <div className="mb-4 flex flex-row gap-4">
            <h2 className="text-fg font-mono text-2xl">{plan.name}</h2>
            {plan.popular && (
              <div className="bg-brand-dark flex h-8 items-center justify-center rounded-full px-4 text-left font-mono text-sm text-white">
                Most Popular
              </div>
            )}
          </div>
          <p className="text-fg mb-4 text-sm opacity-60">{plan.description}</p>

          <div className="flex flex-col gap-2">
            <div className="flex items-baseline gap-2">
              <span className="text-fg font-mono text-4xl font-medium">
                ${plan.price.monthly}
              </span>
              <span className="text-fg-muted">/month</span>
            </div>
            {plan.price.yearly && (
              <div className="text-fg-muted text-sm">
                or ${plan.price.yearly}/year
              </div>
            )}
          </div>
        </div>

        <div className="flex flex-1 flex-col gap-2">
          {plan.features.map((feature, idx) => {
            const IconComponent = feature.included ? CheckCircle2 : XCircle;

            return (
              <div key={idx} className="flex items-start gap-2">
                <IconComponent
                  className={cn([
                    "mt-0.5 size-4.5 shrink-0",
                    feature.included ? "text-green-700" : "text-neutral-300",
                  ])}
                />
                <div className="flex-1">
                  <span
                    className={cn([
                      "text-sm",
                      feature.included
                        ? "text-neutral-900"
                        : "text-neutral-400",
                    ])}
                  >
                    {feature.label}
                  </span>
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

        {isPaid ? (
          <div className="mt-8 rounded-full bg-gradient-to-b from-gray-100 to-gray-700 shadow-sm transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]">
            <a
              href={plan.cta.href}
              className="surface-dark relative flex h-10 w-full items-center justify-center overflow-hidden rounded-full text-sm font-medium text-white"
            >
              <div
                className="pointer-events-none absolute -top-4 left-1/2 h-10 w-full -translate-x-1/2 opacity-40"
                style={{
                  background:
                    "radial-gradient(50% 100% at 50% 0%, white, transparent)",
                }}
              />
              <span className="relative">{plan.cta.label}</span>
            </a>
          </div>
        ) : (
          <a
            href={plan.cta.href}
            className="mt-8 flex h-10 w-full cursor-pointer items-center justify-center rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 text-sm font-medium text-neutral-900 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]"
          >
            {plan.cta.label}
          </a>
        )}
      </div>
    </motion.div>
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
        "Advanced templates let you override Char's default system prompt by configuring template variables and the overall instructions given to the AI.",
    },
    {
      question: "What are shortcuts?",
      answer:
        'Shortcuts are saved prompts you use repeatedly, like "Write a follow-up to blog blah" or "Create a one-pager of the important stuff that\'s been discussed." They\'re available in chat via the / command.',
    },
    {
      question: "Do you offer student discounts?",
      answer:
        "Yes, we provide student discounts. Contact us and we'll help you get set up with student pricing.",
    },
  ];

  return (
    <section className="border-color-brand border-t py-16">
      <div className="flex flex-col gap-6 md:flex-row">
        <h2 className="text-fg mb-4 text-left font-mono text-3xl md:mb-16">
          Frequently Asked Questions
        </h2>
        <div className="flex flex-col gap-6">
          {faqs.map((faq, idx) => (
            <div
              key={idx}
              className="border-color-bright border-b pb-6 last:border-b-0"
            >
              <h3 className="text-fg mb-2 text-lg font-medium">
                {faq.question}
              </h3>
              <p className="text-fg-muted text-base">{faq.answer}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function CTASection() {
  return (
    <section className="laptop:px-0 border-t border-neutral-100 px-4 py-16">
      <div className="flex flex-col items-center gap-6 text-left">
        <h2 className="font-mono text-2xl sm:text-3xl">Need a team plan?</h2>
        <p className="mx-auto max-w-2xl text-lg text-neutral-600">
          Book a call to discuss custom team pricing and enterprise solutions
        </p>
        <div className="rounded-full bg-gradient-to-b from-gray-100 to-gray-700 pt-6 shadow-sm transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]">
          <Link
            to="/founders/"
            search={{ source: "team-plan" }}
            className="surface-dark relative flex h-12 items-center justify-center overflow-hidden rounded-full px-6 text-base font-medium text-white sm:text-lg"
          >
            <div
              className="pointer-events-none absolute -top-4 left-1/2 h-12 w-full -translate-x-1/2 opacity-40"
              style={{
                background:
                  "radial-gradient(50% 100% at 50% 0%, white, transparent)",
              }}
            />
            <span className="relative">Book a call</span>
          </Link>
        </div>
      </div>
    </section>
  );
}
