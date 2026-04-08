import { createFileRoute, Link } from "@tanstack/react-router";
import { motion } from "motion/react";

import { MARKETING_PLAN_TIERS, type MarketingPlanData } from "@hypr/pricing";
import { PlanFeatureList } from "@hypr/pricing/ui";
import { cn } from "@hypr/utils";

import {
  CHAR_SITE_URL,
  getBreadcrumbListJsonLd,
  getSoftwareApplicationJsonLd,
  getStructuredDataGraph,
} from "@/lib/seo";

const PRICING_FAQS = [
  {
    question: "What does on-device transcription mean?",
    answer:
      "The Free plan includes on-device transcription. Lite and Pro can also use Char-hosted cloud transcription when you want managed services instead.",
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
      "A paid plan unlocks Char's cloud layer. Lite gives you hosted transcription, speaker identification, and language models, while Pro adds custom instructions, integrations, sync across devices, and shareable links.",
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
    question: "What are custom instructions?",
    answer:
      "Custom instructions let you override Char's default system prompt by configuring template variables and the overall instructions given to the AI.",
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
] as const;

export const Route = createFileRoute("/_view/pricing")({
  component: Component,
  head: () => {
    const url = `${CHAR_SITE_URL}/pricing`;
    const description =
      "Start free with local transcription, BYOK AI, templates, shortcuts, and chat. Upgrade to Lite or Pro when you want hosted AI, speaker ID, sync, and team features.";

    return {
      links: [{ rel: "canonical", href: url }],
      scripts: [
        {
          type: "application/ld+json",
          children: JSON.stringify(
            getStructuredDataGraph([
              getSoftwareApplicationJsonLd({
                url,
                description,
                aggregateOffer: {
                  lowPrice: 0,
                  highPrice: 25,
                  offerCount: MARKETING_PLAN_TIERS.length,
                },
              }),
              {
                "@type": "FAQPage",
                mainEntity: PRICING_FAQS.map((faq) => ({
                  "@type": "Question",
                  name: faq.question,
                  acceptedAnswer: {
                    "@type": "Answer",
                    text: faq.answer,
                  },
                })),
              },
              getBreadcrumbListJsonLd([
                { name: "Home", item: CHAR_SITE_URL },
                { name: "Pricing", item: url },
              ]),
            ]),
          ),
        },
      ],
      meta: [
        { title: "Pricing - Char" },
        {
          name: "description",
          content: description,
        },
        { property: "og:title", content: "Pricing - Char" },
        {
          property: "og:description",
          content:
            "Compare Char Free, Lite, and Pro. Use local workflows for free, then upgrade when you want managed cloud AI and collaboration features.",
        },
        { property: "og:type", content: "website" },
        { property: "og:url", content: url },
      ],
    };
  },
});

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
          Download the app, then upgrade in desktop when you need cloud
          features.
        </p>
      </div>
    </section>
  );
}

function PricingCardsSection() {
  return (
    <section className="py-16">
      <div className="mx-auto grid grid-cols-1 items-stretch gap-4 md:grid-cols-3">
        {MARKETING_PLAN_TIERS.map((plan) => (
          <PricingCard key={plan.id} plan={plan} />
        ))}
      </div>
    </section>
  );
}

function PricingCard({ plan }: { plan: MarketingPlanData }) {
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
          <p className="text-fg mb-4 min-h-[80px] text-sm opacity-60">
            {plan.description}
          </p>

          <div className="min-h-[64px]">
            {plan.price ? (
              <div className="flex flex-col gap-2">
                <div className="flex items-baseline gap-2">
                  <span className="text-fg font-mono text-4xl font-medium">
                    ${plan.price.monthly}
                  </span>
                  <span className="text-fg-muted">/month</span>
                  {plan.price.yearly != null ? (
                    <span className="text-fg-muted text-sm">
                      or ${plan.price.yearly}/year
                    </span>
                  ) : null}
                </div>
              </div>
            ) : (
              <div className="flex items-baseline gap-2">
                <span className="text-fg font-mono text-4xl font-medium">
                  $0
                </span>
                <span className="text-fg-muted">per month</span>
              </div>
            )}
          </div>
        </div>

        <PlanFeatureList features={plan.features} />

        <div className="mt-auto pt-8">
          <Link
            to="/download/"
            className={cn([
              "flex h-10 w-full cursor-pointer items-center justify-center text-sm font-medium transition-all",
              plan.popular
                ? "rounded-full bg-linear-to-t from-stone-600 to-stone-500 text-white shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]"
                : "rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 shadow-xs hover:scale-[102%] hover:shadow-md active:scale-[98%]",
            ])}
          >
            {plan.price ? "Get Started on Desktop" : "Download for free"}
          </Link>
        </div>
      </div>
    </motion.div>
  );
}

function FAQSection() {
  return (
    <section className="border-color-brand border-t py-16">
      <div className="flex flex-col gap-6 md:flex-row">
        <h2 className="text-fg mb-4 text-left font-mono text-3xl md:mb-16">
          Frequently Asked Questions
        </h2>
        <div className="flex flex-col gap-6">
          {PRICING_FAQS.map((faq, idx) => (
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
