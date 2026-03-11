import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useState } from "react";

import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@hypr/ui/components/ui/accordion";
import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/solution/meeting")({
  component: Component,
  head: () => ({
    meta: [
      { title: "All Your Meeting Notes in One Place - Char" },
      {
        name: "description",
        content:
          "Zoom, Teams, Meet, Discord, or in-person, Char records it all and generates searchable meeting summaries that never leave your device.",
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: "All Your Meeting Notes in One Place - Char",
      },
      {
        property: "og:description",
        content:
          "Real-time transcription, 45+ languages, no bots required. AI-powered meeting notes that stay on your device.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/meeting",
      },
    ],
  }),
});

const heroFeatures = [
  {
    icon: "mdi:microphone",
    title: "Real-Time Transcription",
    description:
      "See conversations transcribed as they happen, while you add your own notes.",
  },
  {
    icon: "mdi:earth",
    title: "45+ Languages",
    description:
      "Multilingual meetings transcribed correctly, without configuration drama.",
  },
  {
    icon: "mdi:robot-off",
    title: "No Bots",
    description:
      "Captures system audio directly—works with any platform, no bot required.",
  },
];

const detailedFeatures = [
  {
    id: "ai-notes",
    title: "AI Notes",
    icon: "mdi:note-edit",
    heading: "Enhance Your Manual Notes with AI",
    description:
      "Combine your notes with AI transcription in one interface. Add context while the AI captures what's said.",
    details:
      "Later, ask AI to generate summaries, action items, or expand on specific topics using both your notes and the transcript. Your manual notes provide context that makes AI summaries more accurate and relevant to your specific needs.",
  },
  {
    id: "search",
    title: "Search",
    icon: "mdi:magnify",
    heading: "Search Across All Your Meetings",
    description:
      "Every word transcribed is searchable. Find that product decision from three months ago or the exact moment someone mentioned a deadline.",
    details:
      "Search by keyword, speaker, or date across all your meeting notes. Char indexes every word, making it instant to find specific conversations, decisions, or action items no matter how long ago they occurred.",
  },
  {
    id: "ai-chat",
    title: "AI Chat",
    icon: "mdi:chat-processing",
    heading: "Chat with Your Meeting Notes",
    description:
      "Ask AI anything about your meeting in natural language—no manual searching required.",
    details:
      '"What were the action items from the product sync?" "When did we discuss the Q4 budget?" "Summarize what the client said about pricing." Get instant answers from your meeting history without having to remember where or when something was discussed.',
  },
  {
    id: "templates",
    title: "Templates",
    icon: "mdi:file-document-multiple",
    heading: "Templates for Every Meeting Type",
    description:
      "Start with pre-built templates for 1:1s, standups, client calls, or interviews.",
    details:
      "Or create your own structure with action items, decisions, and next steps formatted exactly how you want. Templates help you stay organized and ensure you capture the right information every time, whether it's a recurring standup or a one-off client presentation.",
  },
];

const faqs = [
  {
    question: "Does Char work with Zoom, Teams, and Google Meet?",
    answer:
      "Yes. Char works with any application on your computer—Zoom, Teams, Meet, Slack, Discord, and more. It captures system audio directly, so no integrations needed.",
  },
  {
    question: "Do participants know they're being recorded?",
    answer:
      "Char captures audio at the system level on your device—it doesn't join meetings as a bot. Whether participants are notified depends on your meeting platform's settings and your local recording laws. You're responsible for following consent requirements in your jurisdiction.",
  },
  {
    question: "Can I use Char completely offline?",
    answer:
      "Char records audio offline. Cloud transcription requires internet, but you can generate AI summaries offline using local LLM servers like LM Studio or Ollama.",
  },
  {
    question: "Where is my data stored?",
    answer:
      "All recordings and notes are stored locally on your device. Char doesn't send your audio or transcripts to external servers unless you choose cloud transcription.",
  },
  {
    question: "How do I ensure maximum privacy?",
    answer:
      "Use a local LLM server (LM Studio or Ollama) for AI features instead of cloud providers. Keep your recordings stored locally on your device. Review the privacy policies of your chosen STT provider. Review your settings in Settings > Intelligence. When using local LLM servers, your notes and summaries are generated on your device without being sent to external servers.",
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
        <QuickFeaturesSection />
        <DetailedFeaturesSection />
        <DataControlSection />
        <FAQSection />
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
            <Icon icon="mdi:calendar-clock" className="text-lg" />
            <span>For Meetings</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            All Your Meeting Notes
            <br />
            in One Place
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            Zoom, Teams, Meet, Discord, or in-person, Char records it all and
            generates searchable meeting summaries that never leave your device.
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
                "border border-stone-300 text-stone-600",
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

function QuickFeaturesSection() {
  return (
    <section className="border-t border-neutral-100 px-6 py-12">
      <div className="mx-auto grid max-w-4xl gap-6 md:grid-cols-3">
        {heroFeatures.map((feature) => (
          <div
            key={feature.title}
            className="rounded-xl border border-neutral-100 bg-stone-50/50 p-6 text-center"
          >
            <div className="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-xl bg-stone-100">
              <Icon icon={feature.icon} className="text-2xl text-stone-600" />
            </div>
            <h3 className="mb-2 text-base font-medium text-stone-700">
              {feature.title}
            </h3>
            <p className="text-sm text-neutral-600">{feature.description}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

function DetailedFeaturesSection() {
  const [activeFeature, setActiveFeature] = useState(detailedFeatures[0].id);

  const activeFeatureData = detailedFeatures.find(
    (f) => f.id === activeFeature,
  );

  return (
    <section className="border-t border-neutral-100">
      <div className="mx-auto max-w-4xl px-6 py-16">
        <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
          Everything you need for meeting notes
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          AI-powered features that help you capture, organize, and act on every
          conversation.
        </p>
      </div>

      <div className="sticky top-17.25 z-30 border-y border-neutral-100 bg-white/95 backdrop-blur-sm">
        <nav className="mx-auto max-w-4xl px-6">
          <div className="flex gap-1 overflow-x-auto">
            {detailedFeatures.map((feature) => (
              <button
                key={feature.id}
                type="button"
                onClick={() => setActiveFeature(feature.id)}
                className={cn([
                  "flex items-center gap-2 border-b-2 px-4 py-3 text-sm font-medium whitespace-nowrap transition-all",
                  activeFeature === feature.id
                    ? ["border-stone-600 text-stone-700", "bg-stone-50/50"]
                    : [
                        "border-transparent text-neutral-600",
                        "hover:bg-stone-50/30 hover:text-stone-700",
                      ],
                ])}
              >
                <Icon icon={feature.icon} className="text-lg" />
                <span>{feature.title}</span>
              </button>
            ))}
          </div>
        </nav>
      </div>

      {activeFeatureData && (
        <div className="mx-auto max-w-4xl px-6 py-12">
          <div className="grid items-start gap-8 md:grid-cols-2">
            <div>
              <h3 className="mb-4 font-serif text-2xl text-stone-700">
                {activeFeatureData.heading}
              </h3>
              <p className="mb-4 leading-relaxed text-neutral-600">
                {activeFeatureData.description}
              </p>
              <p className="text-sm leading-relaxed text-neutral-600">
                {activeFeatureData.details}
              </p>
            </div>
            <div className="flex aspect-video items-center justify-center rounded-xl border border-neutral-100 bg-stone-100/50">
              <Icon
                icon={activeFeatureData.icon}
                className="text-6xl text-stone-400"
              />
            </div>
          </div>
        </div>
      )}
    </section>
  );
}

function DataControlSection() {
  return (
    <section className="border-t border-neutral-100 bg-stone-50/50 px-6 py-16">
      <div className="mx-auto max-w-4xl">
        <div className="flex gap-6 rounded-xl border border-neutral-100 bg-white p-6">
          <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-stone-100">
            <Icon icon="mdi:shield-lock" className="text-2xl text-stone-600" />
          </div>
          <div>
            <h3 className="mb-3 font-serif text-2xl text-stone-700">
              Control Where Your Data Lives
            </h3>
            <p className="mb-3 leading-relaxed text-neutral-600">
              Run fully local with LM Studio or Ollama for offline AI
              processing. Or connect to cloud providers like Deepgram,
              AssemblyAI, or OpenAI.
            </p>
            <p className="text-sm leading-relaxed text-neutral-600">
              You decide where your data goes, not us. All recordings and notes
              are stored locally on your device by default, and you have full
              control over which services process your data.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}

function FAQSection() {
  return (
    <section className="border-t border-neutral-100 px-6 py-16">
      <div className="mx-auto max-w-4xl">
        <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
          Frequently Asked Questions
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Common questions about using Char for meeting notes.
        </p>
        <Accordion type="single" collapsible className="space-y-2">
          {faqs.map((faq, index) => (
            <AccordionItem
              key={index}
              value={`item-${index}`}
              className="rounded-xl border border-neutral-100 bg-white px-6 data-[state=open]:shadow-sm"
            >
              <AccordionTrigger className="text-lg font-medium text-stone-700 hover:text-stone-900 hover:no-underline">
                {faq.question}
              </AccordionTrigger>
              <AccordionContent className="leading-relaxed text-neutral-600">
                {faq.answer}
              </AccordionContent>
            </AccordionItem>
          ))}
        </Accordion>
      </div>
    </section>
  );
}

function CTASection() {
  return (
    <section className="border-t border-stone-500 bg-linear-to-t from-stone-600 to-stone-500 px-6 py-16">
      <div className="mx-auto max-w-2xl text-center">
        <h2 className="mb-4 font-serif text-3xl text-white">
          Ready to transform your meetings?
        </h2>
        <p className="mb-8 text-stone-100">
          Start capturing every detail with AI-powered meeting notes that
          respect your privacy.
        </p>
        <Link
          to="/download/"
          className={cn([
            "inline-block rounded-full px-8 py-3 text-base font-medium",
            "bg-white text-stone-600",
            "transition-transform hover:scale-105 active:scale-95",
          ])}
        >
          Get started for free
        </Link>
      </div>
    </section>
  );
}
