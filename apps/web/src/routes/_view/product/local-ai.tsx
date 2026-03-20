import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useRef } from "react";

import { cn } from "@hypr/utils";

import { SlashSeparator } from "@/components/slash-separator";
import { CTASection } from "@/routes/_view/index";

export const Route = createFileRoute("/_view/product/local-ai")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Local AI - Char" },
      {
        name: "description",
        content:
          "Powerful AI processing that runs entirely on your device. Private, fast, and offline-capable meeting transcription and summarization with local Whisper and LLM models.",
      },
      { name: "robots", content: "noindex, nofollow" },
      { property: "og:title", content: "Local AI - Char" },
      {
        property: "og:description",
        content:
          "AI-powered meeting notes without cloud uploads. Local transcription with Whisper, on-device summarization, and complete privacy. Works offline with no usage limits.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/product/local-ai",
      },
    ],
  }),
});

function Component() {
  const heroInputRef = useRef<HTMLInputElement>(null);

  return (
    <main
      className="min-h-screen flex-1 bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <HeroSection />
        <SlashSeparator />
        <WhyLocalAISection />
        <SlashSeparator />
        <ComparisonSection />
        <SlashSeparator />
        <CapabilitiesSection />
        <SlashSeparator />
        <ModelsSection />
        <SlashSeparator />
        <ComplianceSection />
        <SlashSeparator />
        <CTASection heroInputRef={heroInputRef} />
      </div>
    </main>
  );
}

function HeroSection() {
  return (
    <section className="bg-linear-to-b from-stone-50/30 to-stone-100/30">
      <div className="flex flex-col items-center gap-6 px-4 py-24 text-center">
        <div className="flex max-w-4xl flex-col gap-6">
          <h1 className="font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            AI that runs
            <br />
            on your device
          </h1>
          <p className="mx-auto max-w-3xl text-lg text-neutral-600 sm:text-xl">
            Char uses powerful local AI models to process your meetings entirely
            on your device. No cloud uploads, complete privacy, and works
            offline.
          </p>
        </div>
        <div className="flex flex-col gap-4 pt-6 sm:flex-row">
          <Link
            to="/download/"
            className={cn([
              "rounded-full px-8 py-3 text-base font-medium",
              "bg-linear-to-t from-stone-600 to-stone-500 text-white",
              "shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
              "transition-all",
            ])}
          >
            Download for free
          </Link>
          <Link
            to="/product/self-hosting/"
            className={cn([
              "rounded-full px-6 py-3 text-base font-medium",
              "border border-neutral-300 text-stone-700",
              "transition-colors hover:bg-stone-50",
            ])}
          >
            Self-hosting options
          </Link>
        </div>
      </div>
    </section>
  );
}

function WhyLocalAISection() {
  return (
    <section>
      <div className="border-b border-neutral-100 text-center">
        <p className="py-6 font-serif font-medium tracking-wide text-neutral-600 uppercase">
          Why local AI
        </p>
      </div>
      <div className="grid md:grid-cols-2">
        <div className="border-r border-b border-neutral-100 p-8 md:border-b-0">
          <Icon
            icon="mdi:shield-lock"
            className="mb-4 text-3xl text-stone-700"
          />
          <h3 className="mb-2 font-serif text-xl text-stone-700">
            Complete privacy
          </h3>
          <p className="text-neutral-600">
            Your meeting recordings, transcripts, and AI-generated summaries
            never leave your computer. Zero data sent to the cloud or
            third-party AI services.
          </p>
        </div>
        <div className="border-b border-neutral-100 p-8 md:border-b-0">
          <Icon
            icon="mdi:lightning-bolt"
            className="mb-4 text-3xl text-stone-700"
          />
          <h3 className="mb-2 font-serif text-xl text-stone-700">
            Lightning fast
          </h3>
          <p className="text-neutral-600">
            No network delays or upload times. AI processing happens instantly
            on your device with optimized local models.
          </p>
        </div>
        <div className="border-r border-neutral-100 p-8">
          <Icon icon="mdi:wifi-off" className="mb-4 text-3xl text-stone-700" />
          <h3 className="mb-2 font-serif text-xl text-stone-700">
            Works offline
          </h3>
          <p className="text-neutral-600">
            Record and process meetings anywhere - on flights, in remote
            locations, or with unreliable internet. No connection required.
          </p>
        </div>
        <div className="p-8">
          <Icon
            icon="mdi:credit-card-off"
            className="mb-4 text-3xl text-stone-700"
          />
          <h3 className="mb-2 font-serif text-xl text-stone-700">
            No usage limits
          </h3>
          <p className="text-neutral-600">
            Process unlimited meetings without worrying about API costs, credit
            limits, or subscription tiers.
          </p>
        </div>
      </div>
    </section>
  );
}

function ComparisonSection() {
  return (
    <section>
      <div className="border-b border-neutral-100 text-center">
        <p className="py-6 font-serif font-medium tracking-wide text-neutral-600 uppercase">
          Local AI vs. Cloud AI
        </p>
      </div>
      <div className="grid md:grid-cols-2">
        <div className="border-r border-neutral-100 p-8">
          <div className="mb-6 flex items-center gap-2">
            <Icon
              icon="mdi:cloud-upload"
              className="text-2xl text-neutral-400"
            />
            <h3 className="font-serif text-lg text-neutral-700">
              Cloud AI Services
            </h3>
          </div>
          <ul className="flex flex-col gap-4 text-neutral-600">
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:close"
                className="mt-0.5 shrink-0 text-neutral-400"
              />
              <span>Audio uploaded to third-party servers</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:close"
                className="mt-0.5 shrink-0 text-neutral-400"
              />
              <span>Requires internet connection</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:close"
                className="mt-0.5 shrink-0 text-neutral-400"
              />
              <span>Processing delays from uploads</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:close"
                className="mt-0.5 shrink-0 text-neutral-400"
              />
              <span>Monthly usage limits</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:close"
                className="mt-0.5 shrink-0 text-neutral-400"
              />
              <span>Data stored on company servers</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:close"
                className="mt-0.5 shrink-0 text-neutral-400"
              />
              <span>Compliance and audit risks</span>
            </li>
          </ul>
        </div>
        <div className="bg-green-50/50 p-8">
          <div className="mb-6 flex items-center gap-2">
            <Icon icon="mdi:laptop" className="text-2xl text-green-600" />
            <h3 className="font-serif text-lg text-green-900">Char Local AI</h3>
          </div>
          <ul className="flex flex-col gap-4 text-green-900">
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:check"
                className="mt-0.5 shrink-0 text-green-600"
              />
              <span>All processing on your device</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:check"
                className="mt-0.5 shrink-0 text-green-600"
              />
              <span>Works completely offline</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:check"
                className="mt-0.5 shrink-0 text-green-600"
              />
              <span>Instant processing, no uploads</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:check"
                className="mt-0.5 shrink-0 text-green-600"
              />
              <span>Unlimited recordings</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:check"
                className="mt-0.5 shrink-0 text-green-600"
              />
              <span>Data never leaves your computer</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:check"
                className="mt-0.5 shrink-0 text-green-600"
              />
              <span>Full compliance control</span>
            </li>
          </ul>
        </div>
      </div>
    </section>
  );
}

function CapabilitiesSection() {
  return (
    <section>
      <div className="border-b border-neutral-100 text-center">
        <p className="py-6 font-serif font-medium tracking-wide text-neutral-600 uppercase">
          Local AI capabilities
        </p>
      </div>
      <div className="grid sm:grid-cols-2 lg:grid-cols-3">
        <div className="border-r border-b border-neutral-100 p-6">
          <Icon icon="mdi:text" className="mb-3 text-2xl text-stone-700" />
          <h3 className="mb-2 font-medium text-stone-700">Transcription</h3>
          <p className="text-sm text-neutral-600">
            High-quality speech-to-text powered by local Whisper models. Support
            for 100+ languages.
          </p>
        </div>
        <div className="border-r border-b border-neutral-100 p-6 lg:border-r">
          <Icon
            icon="mdi:file-document"
            className="mb-3 text-2xl text-stone-700"
          />
          <h3 className="mb-2 font-medium text-stone-700">Summarization</h3>
          <p className="text-sm text-neutral-600">
            AI-generated summaries with key points, decisions, and action items
            using local LLMs.
          </p>
        </div>
        <div className="border-b border-neutral-100 p-6 sm:border-r lg:border-r-0">
          <Icon
            icon="mdi:tag-multiple"
            className="mb-3 text-2xl text-stone-700"
          />
          <h3 className="mb-2 font-medium text-stone-700">Classification</h3>
          <p className="text-sm text-neutral-600">
            Automatic categorization and tagging of conversations by topic and
            meeting type.
          </p>
        </div>
        <div className="border-r border-b border-neutral-100 p-6 sm:border-b-0 lg:border-b-0">
          <Icon icon="mdi:magnify" className="mb-3 text-2xl text-stone-700" />
          <h3 className="mb-2 font-medium text-stone-700">Semantic search</h3>
          <p className="text-sm text-neutral-600">
            Find information across all meetings using natural language with
            local embedding models.
          </p>
        </div>
        <div className="border-r border-neutral-100 p-6">
          <Icon icon="mdi:lightbulb" className="mb-3 text-2xl text-stone-700" />
          <h3 className="mb-2 font-medium text-stone-700">Key insights</h3>
          <p className="text-sm text-neutral-600">
            Extract decisions, questions, and important moments automatically
            from transcripts.
          </p>
        </div>
        <div className="p-6">
          <Icon
            icon="mdi:account-voice"
            className="mb-3 text-2xl text-stone-700"
          />
          <h3 className="mb-2 font-medium text-stone-700">Speaker detection</h3>
          <p className="text-sm text-neutral-600">
            Identify different speakers and attribute quotes accurately with
            diarization.
          </p>
        </div>
      </div>
    </section>
  );
}

function ModelsSection() {
  return (
    <section>
      <div className="border-b border-neutral-100 text-center">
        <p className="py-6 font-serif font-medium tracking-wide text-neutral-600 uppercase">
          AI models we use
        </p>
      </div>
      <div className="divide-y divide-neutral-100">
        <div className="flex items-start gap-4 p-8">
          <Icon
            icon="mdi:microphone"
            className="shrink-0 text-3xl text-stone-700"
          />
          <div>
            <h3 className="mb-2 font-serif text-xl text-stone-700">
              Whisper for transcription
            </h3>
            <p className="text-neutral-600">
              OpenAI's Whisper model running locally on your device.
              Best-in-class accuracy for speech recognition with support for
              100+ languages and robust handling of accents and background
              noise.
            </p>
          </div>
        </div>
        <div className="flex items-start gap-4 p-8">
          <Icon icon="mdi:brain" className="shrink-0 text-3xl text-stone-700" />
          <div>
            <h3 className="mb-2 font-serif text-xl text-stone-700">
              Local LLMs for understanding
            </h3>
            <p className="text-neutral-600">
              Optimized language models for summarization, extraction, and
              analysis. Powerful enough for enterprise-grade results, efficient
              enough to run on your laptop.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}

function ComplianceSection() {
  return (
    <section>
      <div className="border-b border-neutral-100 text-center">
        <p className="py-6 font-serif font-medium tracking-wide text-neutral-600 uppercase">
          Built for compliance
        </p>
      </div>
      <div className="grid md:grid-cols-2">
        <div className="flex gap-4 border-r border-b border-neutral-100 p-8 md:border-b-0">
          <Icon
            icon="mdi:shield-check"
            className="shrink-0 text-3xl text-green-600"
          />
          <div>
            <h3 className="mb-2 font-serif text-lg text-stone-700">
              GDPR & HIPAA ready
            </h3>
            <p className="text-neutral-600">
              Meet data protection requirements by keeping sensitive
              conversations entirely local on user devices.
            </p>
          </div>
        </div>
        <div className="flex gap-4 border-b border-neutral-100 p-8 md:border-b-0">
          <Icon
            icon="mdi:account-lock"
            className="shrink-0 text-3xl text-blue-600"
          />
          <div>
            <h3 className="mb-2 font-serif text-lg text-stone-700">
              Zero data leaks
            </h3>
            <p className="text-neutral-600">
              Eliminate the risk of data breaches, unauthorized access, or
              third-party data mining completely.
            </p>
          </div>
        </div>
        <div className="flex gap-4 border-r border-neutral-100 p-8">
          <Icon
            icon="mdi:file-lock"
            className="shrink-0 text-3xl text-purple-600"
          />
          <div>
            <h3 className="mb-2 font-serif text-lg text-stone-700">
              Full data ownership
            </h3>
            <p className="text-neutral-600">
              Users maintain complete ownership and control. Data stored in
              standard formats on their devices.
            </p>
          </div>
        </div>
        <div className="flex gap-4 p-8">
          <Icon
            icon="mdi:server-off"
            className="shrink-0 text-3xl text-orange-600"
          />
          <div>
            <h3 className="mb-2 font-serif text-lg text-stone-700">
              No vendor lock-in
            </h3>
            <p className="text-neutral-600">
              Data isn't trapped in a proprietary cloud system. Users can export
              and migrate anytime.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
