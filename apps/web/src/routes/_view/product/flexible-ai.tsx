import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

import { FAQ, FAQItem } from "@/components/mdx-jobs";

export const Route = createFileRoute("/_view/product/flexible-ai")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Flexible AI - Char" },
      {
        name: "description",
        content:
          "The only AI note-taker that lets you choose your preferred STT and LLM provider. Cloud, BYOK, or fully local.",
      },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
});

function Component() {
  return (
    <main className="min-h-screen flex-1 md:px-8">
      <div className="mx-auto">
        <HeroSection />
        <AISetupSection />
        <LocalFeaturesSection />
        <SwitchSection />
        <BenchmarkSection />
        <FAQSection />
      </div>
    </main>
  );
}

function HeroSection() {
  return (
    <section className="">
      <div className="flex flex-col items-center gap-6 px-4 py-24 text-left">
        <div className="flex max-w-4xl flex-col gap-6">
          <h1 className="font-mono text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Take Meeting Notes With
            <br />
            AI of Your Choice
          </h1>
          <p className="mx-auto max-w-3xl text-lg text-neutral-600 sm:text-xl">
            The only AI note-taker that lets you choose your preferred STT and
            LLM provider
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
        </div>
      </div>
    </section>
  );
}

function AISetupSection() {
  return (
    <section>
      <div className="border-color-brand border-b text-left">
        <p className="py-6 font-mono font-medium tracking-wide text-neutral-600 uppercase">
          Pick your AI setup
        </p>
      </div>
      <div className="grid md:grid-cols-3">
        <div className="border-color-brand border-r border-b p-8 md:border-b-0">
          <Icon icon="mdi:cloud" className="mb-4 text-3xl text-stone-600" />
          <h3 className="mb-1 font-mono text-xl text-stone-600">
            Char Cloud ($8/month)
          </h3>
          <p className="text-neutral-600">
            Managed service that works out of the box. No setup, no API keys, no
            configuration.
          </p>
        </div>
        <div className="border-color-brand border-r border-b p-8 md:border-b-0">
          <Icon
            icon="mdi:key-variant"
            className="mb-4 text-3xl text-stone-700"
          />
          <h3 className="mb-1 font-mono text-xl text-stone-700">
            Bring Your Own Key (Free)
          </h3>
          <p className="text-neutral-600">
            Use your existing credits from OpenAI, Anthropic, Deepgram, or
            others. No markup.
          </p>
        </div>
        <div className="border-color-brand border-b p-8 md:border-b-0">
          <Icon icon="mdi:laptop" className="mb-4 text-3xl text-stone-700" />
          <h3 className="mb-1 font-mono text-xl text-stone-700">
            Go fully local if you want to
          </h3>
          <p className="text-neutral-600">
            Run everything on your device. Zero data leaves your computer.
          </p>
        </div>
      </div>
    </section>
  );
}

function LocalFeaturesSection() {
  return (
    <section>
      <div className="divide-color-border divide-y">
        <div className="flex items-start gap-4 p-8">
          <Icon
            icon="mdi:microphone"
            className="shrink-0 text-3xl text-stone-700"
          />
          <div>
            <h3 className="mb-2 font-mono text-xl text-stone-700">
              Local transcription with Whisper
            </h3>
            <p className="text-neutral-600">
              Download Whisper models through Ollama or LM Studio. Transcribe
              meetings offline without any API calls.
            </p>
          </div>
        </div>
        <div className="flex items-start gap-4 p-8">
          <Icon icon="mdi:brain" className="shrink-0 text-3xl text-stone-700" />
          <div>
            <h3 className="mb-2 font-mono text-xl text-stone-700">
              Local LLM inference
            </h3>
            <p className="text-neutral-600">
              Run Llama 3, Mistral, Qwen, or other open-source models locally
              for AI summaries and chat.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}

function SwitchSection() {
  return (
    <section>
      <div className="border-b border-neutral-100 text-left">
        <p className="py-6 font-mono font-medium tracking-wide text-neutral-600 uppercase">
          Switch providers anytime
        </p>
      </div>
      <p className="border-b border-neutral-100 px-4 py-6 text-left text-neutral-600">
        Your notes aren't locked to any AI provider.
      </p>
      <div className="grid md:grid-cols-2">
        <div className="border-r border-b border-neutral-100 p-8 md:border-b-0">
          <h3 className="mb-2 font-mono text-lg text-stone-700">
            Start with Cloud
          </h3>
          <p className="text-neutral-600">
            Try Char's managed service free for 14 days.
          </p>
        </div>
        <div className="border-b border-neutral-100 p-8 md:border-b-0">
          <h3 className="mb-2 font-mono text-lg text-stone-700">
            Change based on needs
          </h3>
          <p className="text-neutral-600">
            Go local for sensitive discussions. Cloud for more power. BYOK for
            API cost control.
          </p>
        </div>
        <div className="border-r border-neutral-100 p-8">
          <h3 className="mb-2 font-mono text-lg text-stone-700">
            Re-process meetings
          </h3>
          <p className="text-neutral-600">
            Run new models on old transcripts when better AI launches.
          </p>
        </div>
        <div className="p-8">
          <h3 className="mb-2 font-mono text-lg text-stone-700">
            Data never moves
          </h3>
          <p className="text-neutral-600">
            Notes stay on your device. Only the AI layer changes.
          </p>
        </div>
      </div>
    </section>
  );
}

function BenchmarkSection() {
  return (
    <section className="bg-linear-to-b from-stone-50/30 to-stone-100/30">
      <div className="flex flex-col items-center gap-6 px-4 py-16 text-left">
        <h2 className="font-mono text-2xl text-stone-700 sm:text-3xl">
          Confused which AI model to choose?
        </h2>
        <p className="mx-auto max-w-2xl text-neutral-600">
          We benchmark leading AI models on real meeting
          tasks&mdash;summarization, Q&A, action items, and speaker ID. See
          detailed comparisons to find the right fit.
        </p>
        <Link
          to="/eval/"
          className={cn([
            "rounded-full px-8 py-3 text-base font-medium",
            "border border-neutral-300 text-stone-700",
            "transition-colors hover:bg-stone-50",
          ])}
        >
          View AI model evaluations
        </Link>
      </div>
    </section>
  );
}

function FAQSection() {
  return (
    <section className="px-4 py-16">
      <div className="mx-auto max-w-4xl">
        <div className="mb-12 text-left">
          <h2 className="font-mono text-3xl text-stone-700">
            Frequently asked questions
          </h2>
        </div>
        <FAQ>
          <FAQItem question="Which AI models does Char use?">
            Char Cloud routes requests to the best models for each task.
          </FAQItem>
          <FAQItem question="Can I use different models for different meetings?">
            Yes. You can switch providers before any meeting or re-process
            existing transcripts with different models anytime.
          </FAQItem>
          <FAQItem question="What happens to my notes if I switch providers?">
            Nothing. Your notes are Markdown files on your device. Switching AI
            providers doesn't affect your data at all.
          </FAQItem>
          <FAQItem question="Is local AI good enough?">
            Local models are improving rapidly. For most meetings, local Whisper
            + Llama 3 works well. For complex summaries or technical
            discussions, cloud models (Char Cloud or BYOK) tend to perform
            better.
          </FAQItem>
          <FAQItem question="Does Char train AI models on my data?">
            No. Char does not use your recordings, transcripts, or notes to
            train AI models. When using cloud providers, your data is processed
            according to their privacy policies.
          </FAQItem>
        </FAQ>
      </div>
    </section>
  );
}
