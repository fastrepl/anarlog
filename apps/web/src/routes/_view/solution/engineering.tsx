import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@hypr/ui/components/ui/accordion";
import { cn } from "@hypr/utils";

import { SlashSeparator } from "@/components/slash-separator";

export const Route = createFileRoute("/_view/solution/engineering")({
  component: Component,
  head: () => ({
    meta: [
      {
        title:
          "For Developers - The Only Meeting AI You Can Fork, Fix & Make Your Own",
      },
      {
        name: "description",
        content:
          "Build React extensions, automate with shell hooks, bring your own keys. Self-host or run local. No proprietary modules, just open source code you can inspect and modify.",
      },
      {
        property: "og:title",
        content:
          "For Developers - The Only Meeting AI You Can Fork, Fix & Make Your Own",
      },
      {
        property: "og:description",
        content:
          "Build React extensions, automate with shell hooks, bring your own keys. Self-host or run local. No proprietary modules, just open source code you can inspect and modify.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/solution/engineering",
      },
      { name: "twitter:card", content: "summary_large_image" },
      {
        name: "twitter:title",
        content:
          "For Developers - The Only Meeting AI You Can Fork, Fix & Make Your Own",
      },
      {
        name: "twitter:description",
        content:
          "Build React extensions, automate with shell hooks, bring your own keys. Self-host or run local.",
      },
      {
        name: "keywords",
        content:
          "open source, meeting notes, AI transcription, developer tools, Rust, Tauri, React, TypeScript, BYOK, local AI, whisper, llm, API, CLI, extensions",
      },
    ],
  }),
});

const technicalSpecs = [
  {
    feature: "Languages",
    details: "Rust (backend), TypeScript/React (frontend)",
  },
  {
    feature: "Build System",
    details: "Taskfile for task management, pnpm for packages",
  },
  { feature: "Desktop Framework", details: "Tauri (cross-platform)" },
  {
    feature: "Database",
    details: "SQLite (local), optional Supabase integration",
  },
  {
    feature: "Extension Runtime",
    details: "Sandboxed Deno for scripts, React for UI",
  },
  {
    feature: "Supported Platforms",
    details: "macOS (stable), Linux (via Flatpak), Windows (coming soon)",
  },
  {
    feature: "AI Providers",
    details:
      "OpenAI, Anthropic, Deepgram, AssemblyAI, Soniox, Fireworks, Gladia, or local via LM Studio/Ollama",
  },
  { feature: "License", details: "GPL v3.0" },
];

const byokFeatures = [
  {
    icon: "mdi:key-variant",
    title: "STT Provider Keys",
    description:
      "Connect your own API keys for STT providers: Deepgram, AssemblyAI, Soniox, OpenAI Whisper",
  },
  {
    icon: "mdi:brain",
    title: "LLM Endpoints",
    description:
      "Use your own LLM endpoints: OpenAI, Anthropic, or any OpenAI-compatible API",
  },
  {
    icon: "mdi:laptop",
    title: "Run Local",
    description:
      "Or skip keys entirely, run local models via LM Studio/Ollama with zero cloud dependency",
  },
  {
    icon: "mdi:currency-usd-off",
    title: "Free Forever",
    description:
      "Free tier runs forever with BYOK, upgrade to Pro only if you want our managed cloud services",
  },
];

const automationHooksFeatures = [
  {
    icon: "mdi:script-text",
    title: "Shell Scripts",
    description:
      "Shell scripts triggered automatically when recordings start or stop",
  },
  {
    icon: "mdi:code-json",
    title: "Session Metadata",
    description:
      "Receive session metadata as CLI arguments: resource directory, app identifiers, meeting context",
  },
  {
    icon: "mdi:cog-sync",
    title: "External Integration",
    description:
      "Trigger file processing, update external systems, or integrate with tools we've never heard of",
  },
  {
    icon: "mdi:timer-off",
    title: "No Polling",
    description:
      "No polling, no webhooks to configure, just commands that run when events happen",
  },
];

const extensibilityFeatures = [
  {
    icon: "mdi:react",
    title: "Custom UI Panels",
    description:
      "Build custom UI panels using React and our component library (@hypr/ui)",
  },
  {
    icon: "mdi:database-sync",
    title: "Synchronized State",
    description:
      "Access synchronized app state via TinyBase: sessions, calendar events, contacts, transcripts",
  },
  {
    icon: "mdi:tab-plus",
    title: "Programmatic Control",
    description:
      "Open tabs programmatically, create custom workflows, or build entirely new interfaces",
  },
  {
    icon: "mdi:shield-check",
    title: "Sandboxed Security",
    description:
      "Extensions run in sandboxed iframes with validated security boundaries",
  },
];

const cliFeatures = [
  {
    icon: "mdi:export",
    title: "Export & Manage",
    description:
      "Export sessions, manage data, trigger transcriptions, all from the command line",
  },
  {
    icon: "mdi:pipe",
    title: "Scripting Ready",
    description:
      "Designed for scripting: pipe output to other tools, run from cron jobs, embed in CI/CD",
  },
  {
    icon: "mdi:folder-multiple",
    title: "Session Management",
    description:
      "Full session management: list, export, delete, create from audio files",
  },
  {
    icon: "mdi:console",
    title: "Automation Interface",
    description: "Your automation interface when the GUI isn't enough",
  },
];

const apiFeatures = [
  {
    icon: "mdi:api",
    title: "REST API",
    description:
      "REST API for programmatic control over sessions, transcripts, and AI processing",
  },
  {
    icon: "mdi:webhook",
    title: "CRUD & Webhooks",
    description:
      "CRUD operations, webhook support, AI trigger endpoints—all documented upfront",
  },
  {
    icon: "mdi:office-building",
    title: "Enterprise Ready",
    description: "Built for enterprise integrations and custom tooling",
  },
];

const faqs = [
  {
    question: "Can I build commercial products on Char?",
    answer:
      "Yes. The open source license permits commercial use. Build integrations, sell extensions, or embed Char in your own products—no licensing fees, no revenue sharing requirements.",
  },
  {
    question: "Is the entire codebase open?",
    answer:
      'Yes. Complete stack on GitHub—Rust backend, TypeScript/React frontend, extension runtime, build system. No proprietary modules, no "core" that\'s closed source, no enterprise features in private repos.',
  },
  {
    question: "Can I run it completely offline?",
    answer:
      "Yes. Download Whisper models once, run a local LLM server (LM Studio/Ollama), and Char never needs internet. Audio processing, transcription, AI summaries—all on-device. Perfect for air-gapped environments or when you don't trust cloud providers.",
  },
  {
    question: "What's the extension security model?",
    answer:
      "Extensions run in iframes with sandbox=\"allow-scripts\". They can't access parent DOM, can't invoke Tauri APIs (we polyfill __TAURI_INTERNALS__ to reject all calls), and communicate only through validated postMessage. Script URLs verified, content-types checked, exports validated before rendering. Designed to let you extend functionality without compromising the security boundary.",
  },
  {
    question: "Do you train AI models on my data?",
    answer:
      "No. Char doesn't collect transcripts, recordings, or notes for training. When you use cloud STT providers, your data is processed per their privacy policies (most don't retain audio post-transcription), but Char itself never trains on your data.",
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
        <SlashSeparator />
        <TechnicalSpecsSection />
        <SlashSeparator />
        <BYOKSection />
        <SlashSeparator />
        <AutomationHooksSection />
        <SlashSeparator />
        <ExtensibilitySection />
        <SlashSeparator />
        <CLISection />
        <SlashSeparator />
        <APISection />
        <SlashSeparator />
        <FAQSection />
        <SlashSeparator />
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
            <Icon icon="mdi:code-braces" className="text-lg" />
            <span>For Developers</span>
          </div>
          <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            The Only Meeting AI You Can
            <br />
            Fork, Fix & Make Your Own
          </h1>
          <p className="mx-auto max-w-2xl text-lg text-neutral-600 sm:text-xl">
            Build React extensions, automate with shell hooks, bring your own
            keys. Self-host or run local. No proprietary modules, just open
            source code you can inspect and modify.
          </p>
          <div className="mt-8 flex flex-col justify-center gap-4 sm:flex-row">
            <a
              href="https://github.com/fastrepl/char"
              target="_blank"
              rel="noopener noreferrer"
              className={cn([
                "inline-flex items-center justify-center gap-2 rounded-full px-8 py-3 text-base font-medium",
                "bg-linear-to-t from-neutral-800 to-neutral-700 text-white",
                "transition-transform hover:scale-105 active:scale-95",
              ])}
            >
              <Icon icon="mdi:github" className="text-lg" />
              Inspect the Code
            </a>
            <Link
              to="/download/"
              className={cn([
                "inline-block rounded-full px-8 py-3 text-base font-medium",
                "border border-stone-300 text-stone-600",
                "transition-colors hover:bg-stone-50",
              ])}
            >
              Download Char
            </Link>
          </div>
        </header>
      </div>
    </div>
  );
}

function TechnicalSpecsSection() {
  return (
    <section className="px-6 py-16">
      <div className="mx-auto max-w-4xl">
        <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
          Technical Specs
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Built with modern, privacy-respecting technologies that run locally on
          your device.
        </p>
        <div className="overflow-hidden rounded-xl border border-neutral-100">
          <table className="w-full">
            <thead>
              <tr className="bg-stone-50/50">
                <th className="border-b border-neutral-100 px-6 py-4 text-left text-sm font-medium text-stone-700">
                  Feature
                </th>
                <th className="border-b border-neutral-100 px-6 py-4 text-left text-sm font-medium text-stone-700">
                  Details
                </th>
              </tr>
            </thead>
            <tbody>
              {technicalSpecs.map((spec, index) => (
                <tr
                  key={spec.feature}
                  className={cn([
                    index % 2 === 0 ? "bg-white" : "bg-stone-50/30",
                    index < technicalSpecs.length - 1 &&
                      "border-b border-neutral-100",
                  ])}
                >
                  <td className="px-6 py-4 text-sm font-medium text-stone-700">
                    {spec.feature}
                  </td>
                  <td className="px-6 py-4 text-sm text-neutral-600">
                    {spec.details}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}

function FeatureGrid({
  features,
}: {
  features: Array<{ icon: string; title: string; description: string }>;
}) {
  return (
    <div className="mx-auto grid max-w-4xl gap-6 md:grid-cols-2">
      {features.map((feature) => (
        <div
          key={feature.title}
          className="rounded-xl border border-neutral-100 bg-stone-50/50 p-6"
        >
          <div className="mb-4 flex h-12 w-12 items-center justify-center rounded-xl bg-stone-100">
            <Icon icon={feature.icon} className="text-2xl text-stone-600" />
          </div>
          <h3 className="mb-2 text-base font-medium text-stone-700">
            {feature.title}
          </h3>
          <p className="text-sm text-neutral-600">{feature.description}</p>
        </div>
      ))}
    </div>
  );
}

function BYOKSection() {
  return (
    <section className="px-6 py-16">
      <div className="mx-auto max-w-4xl">
        <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
          Bring Your Own Key
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Use your preferred AI providers or run everything locally. No vendor
          lock-in, no forced subscriptions.
        </p>
        <FeatureGrid features={byokFeatures} />
      </div>
    </section>
  );
}

function AutomationHooksSection() {
  return (
    <section className="bg-stone-50/30 px-6 py-16">
      <div className="mx-auto max-w-4xl">
        <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
          Automation Hooks
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Trigger custom workflows when events happen. No polling, no
          webhooks—just shell scripts that run automatically.
        </p>
        <FeatureGrid features={automationHooksFeatures} />
      </div>
    </section>
  );
}

function ExtensibilitySection() {
  return (
    <section className="px-6 py-16">
      <div className="mx-auto max-w-4xl">
        <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
          Fully Extensible
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Build custom UI panels, access app state, and create entirely new
          workflows with our extension system.
        </p>
        <FeatureGrid features={extensibilityFeatures} />
      </div>
    </section>
  );
}

function CLISection() {
  return (
    <section className="bg-stone-50/30 px-6 py-16">
      <div className="mx-auto max-w-4xl">
        <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
          Cross-Platform CLI
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Manage sessions, export data, and automate workflows from the command
          line.
        </p>
        <FeatureGrid features={cliFeatures} />
      </div>
    </section>
  );
}

function APISection() {
  return (
    <section className="px-6 py-16">
      <div className="mx-auto max-w-4xl">
        <div className="mb-12 text-center">
          <h2 className="mb-4 font-serif text-3xl text-stone-700">
            API Access
          </h2>
          <p className="mx-auto max-w-2xl text-neutral-600">
            Programmatic control over sessions, transcripts, and AI processing
            for enterprise integrations.
          </p>
          <div className="mt-4 inline-flex items-center gap-2 rounded-full bg-amber-50 px-4 py-2 text-sm text-amber-700">
            <Icon icon="mdi:clock-outline" className="text-lg" />
            <span>Coming Soon</span>
          </div>
        </div>
        <div className="grid gap-6 md:grid-cols-3">
          {apiFeatures.map((feature) => (
            <div
              key={feature.title}
              className="rounded-xl border border-neutral-100 bg-stone-50/50 p-6 opacity-75"
            >
              <div className="mb-4 flex h-12 w-12 items-center justify-center rounded-xl bg-stone-100">
                <Icon icon={feature.icon} className="text-2xl text-stone-400" />
              </div>
              <h3 className="mb-2 text-base font-medium text-stone-500">
                {feature.title}
              </h3>
              <p className="text-sm text-neutral-500">{feature.description}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function FAQSection() {
  return (
    <section className="px-6 py-16">
      <div className="mx-auto max-w-4xl">
        <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
          Frequently Asked Questions
        </h2>
        <p className="mx-auto mb-12 max-w-2xl text-center text-neutral-600">
          Common questions about building with Char.
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
        <h2 className="mb-4 font-serif text-3xl text-white">Ready to build?</h2>
        <p className="mb-8 text-stone-100">
          Fork the repo, explore the codebase, and start building your own
          meeting AI.
        </p>
        <div className="flex flex-col justify-center gap-4 sm:flex-row">
          <a
            href="https://github.com/fastrepl/char"
            target="_blank"
            rel="noopener noreferrer"
            className={cn([
              "inline-flex items-center justify-center gap-2 rounded-full px-8 py-3 text-base font-medium",
              "bg-white text-stone-600",
              "transition-transform hover:scale-105 active:scale-95",
            ])}
          >
            <Icon icon="mdi:github" className="text-lg" />
            View on GitHub
          </a>
          <Link
            to="/download/"
            className={cn([
              "inline-block rounded-full px-8 py-3 text-base font-medium",
              "border border-stone-300 text-white",
              "transition-colors hover:bg-stone-500/50",
            ])}
          >
            Download Char
          </Link>
        </div>
      </div>
    </section>
  );
}
