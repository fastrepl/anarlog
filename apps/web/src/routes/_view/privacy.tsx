import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/privacy")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Privacy - Char" },
      {
        name: "description",
        content:
          "Char captures meetings and activity on your computer—all processed locally, never uploaded. Privacy isn't a feature, it's the foundation.",
      },
      { property: "og:title", content: "Privacy - Char" },
      {
        property: "og:description",
        content:
          "Char captures your meetings and activity—all locally on your device. Your data never leaves unless you say so.",
      },
      { property: "og:type", content: "website" },
      { property: "og:url", content: "https://char.com/privacy" },
      { name: "twitter:card", content: "summary_large_image" },
      { name: "twitter:title", content: "Privacy - Char" },
      {
        name: "twitter:description",
        content:
          "Char captures your meetings and activity—all locally on your device. Your data never leaves unless you say so.",
      },
    ],
  }),
});

function Component() {
  return (
    <div className="min-h-screen">
      <div className="mx-auto">
        <HeroSection />
        <PrivacyPromiseSection />
        <DataOwnershipSection />
        <NoTrackingSection />
        <TransparencySection />
        <PrivacyComparisonSection />
        <CTASection />
      </div>
    </div>
  );
}

function HeroSection() {
  return (
    <div className="bg-linear-to-b from-stone-50/30 to-stone-100/30">
      <div className="px-6 py-12 lg:py-20">
        <header className="mx-auto mb-12 max-w-4xl text-left">
          <div className="mb-6 inline-flex items-center gap-2 rounded-full bg-stone-100 px-4 py-2 text-sm text-stone-600">
            <Icon icon="mdi:shield-check" className="text-lg" />
            <span>Privacy by nature</span>
          </div>
          <h1 className="mb-6 font-mono text-4xl text-stone-600 sm:text-5xl lg:text-6xl">
            Your data lives
            <br />
            on your device
          </h1>
          <p className="mx-auto max-w-3xl text-lg leading-relaxed text-neutral-600 sm:text-xl">
            Char captures meetings, tracks activity on your computer, and
            manages your daily notes—all locally. We're integrating deep context
            capture (like a keylogger) because the more Char knows, the more it
            can help. But that level of access demands real privacy, not just
            promises. That's why everything runs on your device.
          </p>
        </header>
      </div>
    </div>
  );
}

function PrivacyPromiseSection() {
  const promises = [
    {
      icon: "mdi:laptop",
      title: "Everything stays local",
      description:
        "Meetings, activity capture, transcripts, and notes are all processed and stored on your device. Nothing is uploaded to our servers.",
    },
    {
      icon: "mdi:brain",
      title: "On-device AI",
      description:
        "Run AI models locally for transcription and note enhancement. No internet required. Your data never touches a cloud API unless you choose to use one.",
    },
    {
      icon: "mdi:database-off",
      title: "No data collection",
      description:
        "We don't collect, analyze, or monetize your data—meetings, activity, or otherwise. Nothing is used to train AI models or sold to third parties.",
    },
    {
      icon: "mdi:code-braces",
      title: "Open source & verifiable",
      description:
        "Every line of code is public. You can verify exactly how activity capture works, what data is stored, and that nothing leaves your machine.",
    },
  ];

  return (
    <section className="px-6 py-12 lg:py-16">
      <h2 className="mb-4 text-left font-mono text-3xl text-stone-600">
        Privacy by nature, not by promise
      </h2>
      <p className="mx-auto mb-12 max-w-2xl text-left text-neutral-600">
        These aren't policies—they're architectural decisions. Char physically
        cannot send your data to us because it never leaves your device.
      </p>
      <div className="mx-auto grid max-w-4xl gap-8 md:grid-cols-2">
        {promises.map((promise, index) => (
          <div
            key={index}
            className="rounded-lg border border-neutral-200 bg-white p-6"
          >
            <Icon
              icon={promise.icon}
              className="mb-4 text-3xl text-stone-600"
            />
            <h3 className="mb-2 font-mono text-xl text-stone-600">
              {promise.title}
            </h3>
            <p className="text-neutral-600">{promise.description}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

function DataOwnershipSection() {
  return (
    <section className="bg-stone-50/30 px-6 py-12 lg:py-16">
      <div className="mx-auto max-w-4xl">
        <div className="mb-12 text-left">
          <Icon
            icon="mdi:folder-lock"
            className="mb-4 text-5xl text-stone-600"
          />
          <h2 className="mb-4 font-mono text-3xl text-stone-600">
            Ownership means it's on your device
          </h2>
          <p className="mx-auto max-w-2xl text-neutral-600">
            Meetings, activity logs, daily notes—all stored locally in formats
            you control. True ownership, not a marketing term.
          </p>
        </div>

        <div className="grid gap-6 md:grid-cols-3">
          <div className="p-6 text-left">
            <Icon
              icon="mdi:folder-home"
              className="mx-auto mb-4 text-4xl text-stone-600"
            />
            <h3 className="mb-2 font-medium text-stone-600">Local storage</h3>
            <p className="text-sm text-neutral-600">
              All your notes, recordings, activity data, and transcripts are
              stored locally on your computer. No cloud dependency.
            </p>
          </div>
          <div className="p-6 text-left">
            <Icon
              icon="mdi:export"
              className="mx-auto mb-4 text-4xl text-stone-600"
            />
            <h3 className="mb-2 font-medium text-stone-600">Full export</h3>
            <p className="text-sm text-neutral-600">
              Export all your data anytime in standard formats. Your notes,
              transcripts, and recordings are always accessible and portable.
            </p>
          </div>
          <div className="p-6 text-left">
            <Icon
              icon="mdi:delete-forever"
              className="mx-auto mb-4 text-4xl text-stone-600"
            />
            <h3 className="mb-2 font-medium text-stone-600">True deletion</h3>
            <p className="text-sm text-neutral-600">
              When you delete something, it's gone. No hidden backups, no
              retention periods, no "soft deletes" that keep your data around.
            </p>
          </div>
        </div>

        <div className="mt-12 rounded-lg border border-neutral-200 bg-white p-8">
          <div className="flex items-start gap-4">
            <Icon
              icon="mdi:sync-off"
              className="shrink-0 text-3xl text-stone-600"
            />
            <div>
              <h3 className="mb-3 font-mono text-xl text-stone-600">
                Optional sync, your choice
              </h3>
              <p className="text-neutral-600">
                If you choose to sync across devices, your data is encrypted
                before it leaves your device. We use end-to-end encryption so
                even our servers can't read your content. But sync is entirely
                optional—Char works perfectly as a standalone, offline
                application.
              </p>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function NoTrackingSection() {
  return (
    <section className="px-6 py-12 lg:py-16">
      <div className="mx-auto max-w-4xl">
        <div className="mb-12 text-left">
          <Icon
            icon="mdi:shield-off-outline"
            className="mb-4 text-5xl text-stone-600"
          />
          <h2 className="mb-4 font-mono text-3xl text-stone-600">
            We don't see your data. Period.
          </h2>
          <p className="mx-auto max-w-2xl text-neutral-600">
            Char captures deep context about your work—but we never see it. Your
            activity, meetings, and notes are not our product.
          </p>
        </div>

        <div className="flex flex-col gap-6">
          <div className="rounded-lg border border-neutral-200 bg-white p-6">
            <div className="flex items-start gap-4">
              <Icon
                icon="mdi:robot-off"
                className="mt-1 shrink-0 text-2xl text-stone-600"
              />
              <div>
                <h3 className="mb-2 font-medium text-stone-600">
                  No AI training on your data
                </h3>
                <p className="text-neutral-600">
                  Your transcripts, activity data, and notes are never used to
                  train AI models. On-device AI means your content stays on your
                  machine.
                </p>
              </div>
            </div>
          </div>

          <div className="rounded-lg border border-neutral-200 bg-white p-6">
            <div className="flex items-start gap-4">
              <Icon
                icon="mdi:target-account"
                className="mt-1 shrink-0 text-2xl text-stone-600"
              />
              <div>
                <h3 className="mb-2 font-medium text-stone-600">
                  No behavioral tracking
                </h3>
                <p className="text-neutral-600">
                  We don't track how you use the app, what features you access,
                  or how long you spend on different tasks. Your usage patterns
                  are your business.
                </p>
              </div>
            </div>
          </div>

          <div className="rounded-lg border border-neutral-200 bg-white p-6">
            <div className="flex items-start gap-4">
              <Icon
                icon="mdi:currency-usd-off"
                className="mt-1 shrink-0 text-2xl text-stone-600"
              />
              <div>
                <h3 className="mb-2 font-medium text-stone-600">
                  No data monetization
                </h3>
                <p className="text-neutral-600">
                  We make money by building a great product, not by selling your
                  data. Your information is never shared with advertisers, data
                  brokers, or any third parties.
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function TransparencySection() {
  return (
    <section className="bg-stone-50/30 px-6 py-12 lg:py-16">
      <div className="mx-auto max-w-4xl">
        <div className="mb-12 text-left">
          <Icon
            icon="mdi:code-braces"
            className="mb-4 text-5xl text-stone-600"
          />
          <h2 className="mb-4 font-mono text-3xl text-stone-600">
            Verify, don't trust
          </h2>
          <p className="mx-auto max-w-2xl text-neutral-600">
            We're asking for deep access to your work. That's why we don't ask
            you to trust us—we ask you to verify. Char is fully open source.
          </p>
        </div>

        <div className="grid gap-8 md:grid-cols-2">
          <div className="rounded-lg border border-neutral-200 bg-white p-6">
            <Icon
              icon="mdi:source-repository"
              className="mb-4 text-3xl text-stone-600"
            />
            <h3 className="mb-2 font-mono text-xl text-stone-600">
              Open source code
            </h3>
            <p className="text-neutral-600">
              Every line of code is public. See exactly how your data is
              handled, what network requests are made, and where your
              information is stored. No black boxes, no hidden behaviors.
            </p>
          </div>
          <div className="rounded-lg border border-neutral-200 bg-white p-6">
            <Icon
              icon="mdi:file-document-check"
              className="mb-4 text-3xl text-stone-600"
            />
            <h3 className="mb-2 font-mono text-xl text-stone-600">
              Clear documentation
            </h3>
            <p className="text-neutral-600">
              Our privacy practices are documented in plain language. We explain
              what data exists, where it lives, and how it's protected—without
              legal jargon or hidden clauses.
            </p>
          </div>
        </div>

        <div className="mt-8 flex flex-col justify-center gap-4 sm:flex-row">
          <a
            href="https://github.com/fastrepl/char"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center justify-center gap-2 font-medium text-stone-600 hover:text-stone-800"
          >
            <Icon icon="mdi:github" className="text-lg" />
            View source code
            <Icon icon="mdi:arrow-right" className="text-lg" />
          </a>
          <a
            href="/legal/privacy"
            className="inline-flex items-center justify-center gap-2 font-medium text-stone-600 hover:text-stone-800"
          >
            <Icon icon="mdi:file-document" className="text-lg" />
            Read privacy policy
            <Icon icon="mdi:arrow-right" className="text-lg" />
          </a>
        </div>
      </div>
    </section>
  );
}

function PrivacyComparisonSection() {
  const comparisons = [
    {
      feature: "Audio processing",
      hyprnote: "On your device",
      others: "Cloud servers",
    },
    {
      feature: "Data storage",
      hyprnote: "Local only",
      others: "Their servers",
    },
    {
      feature: "AI training",
      hyprnote: "Never",
      others: "Often",
    },
    {
      feature: "Account required",
      hyprnote: "No",
      others: "Yes",
    },
    {
      feature: "Data monetization",
      hyprnote: "Never",
      others: "Common",
    },
    {
      feature: "Source code",
      hyprnote: "Open",
      others: "Closed",
    },
  ];

  return (
    <section className="px-6 py-12 lg:py-16">
      <div className="mx-auto max-w-4xl">
        <div className="mb-12 text-left">
          <h2 className="mb-4 font-mono text-3xl text-stone-600">
            How we compare
          </h2>
          <p className="mx-auto max-w-2xl text-neutral-600">
            Most tools treat your data as their asset. Char is built so your
            data physically cannot reach us.
          </p>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full border-collapse">
            <thead>
              <tr className="border-b border-neutral-200">
                <th className="px-4 py-4 text-left font-medium text-stone-600">
                  Feature
                </th>
                <th className="bg-stone-50 px-4 py-4 text-left font-medium text-stone-600">
                  Char
                </th>
                <th className="px-4 py-4 text-left font-medium text-neutral-500">
                  Others
                </th>
              </tr>
            </thead>
            <tbody>
              {comparisons.map((row, index) => (
                <tr key={index} className="border-b border-neutral-100">
                  <td className="px-4 py-4 text-neutral-600">{row.feature}</td>
                  <td className="bg-stone-50 px-4 py-4 text-left">
                    <span className="inline-flex items-center gap-2 font-medium text-stone-600">
                      <Icon
                        icon="mdi:check-circle"
                        className="text-lg text-green-600"
                      />
                      {row.hyprnote}
                    </span>
                  </td>
                  <td className="px-4 py-4 text-left text-neutral-500">
                    {row.others}
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

function CTASection() {
  return (
    <section className="bg-stone-50/30 px-6 py-16 lg:py-20">
      <div className="mx-auto max-w-3xl text-left">
        <Icon
          icon="mdi:shield-lock"
          className="mx-auto mb-6 text-5xl text-stone-600"
        />
        <h2 className="mb-4 font-mono text-3xl text-stone-600">
          Deep context. Complete privacy.
        </h2>
        <p className="mb-8 text-neutral-600">
          The more Char knows about your work, the more it can help. And because
          everything runs locally, that's a tradeoff you can actually make.
        </p>
        <div className="flex flex-col justify-center gap-4 sm:flex-row">
          <Link
            to="/download/"
            className={cn([
              "inline-flex items-center justify-center gap-2 rounded-full px-8 py-3 text-base font-medium",
              "bg-linear-to-t from-stone-600 to-stone-500 text-white",
              "shadow-md transition-transform hover:scale-105 hover:shadow-lg active:scale-95",
            ])}
          >
            <Icon icon="mdi:download" className="text-lg" />
            Download Char
          </Link>
          <Link
            to="/security/"
            className={cn([
              "inline-flex items-center justify-center rounded-full px-8 py-3 text-base font-medium",
              "border border-neutral-300 text-stone-600",
              "transition-colors hover:bg-stone-50",
            ])}
          >
            Learn about security
          </Link>
        </div>
      </div>
    </section>
  );
}
