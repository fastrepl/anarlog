import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/security")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Security - Char" },
      {
        name: "description",
        content:
          "Char keeps core meeting data local by default, supports self-hosted and local AI workflows, and documents what leaves your device when you enable cloud features.",
      },
      { property: "og:title", content: "Security - Char" },
      {
        property: "og:description",
        content:
          "See how Char handles local files, cloud providers, open source auditing, and security controls.",
      },
      { property: "og:type", content: "website" },
      { property: "og:url", content: "https://char.com/security" },
      { name: "twitter:card", content: "summary_large_image" },
      { name: "twitter:title", content: "Security - Char" },
      {
        name: "twitter:description",
        content:
          "See how Char handles local files, cloud providers, open source auditing, and security controls.",
      },
    ],
  }),
});

function Component() {
  return (
    <div className="min-h-screen">
      <div className="mx-auto">
        <HeroSection />
        <SecurityPrinciplesSection />
        <LocalFirstSection />
        <EncryptionSection />
        <OpenSourceSecuritySection />
        <EnterpriseSecuritySection />
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
          <div className="mb-6 inline-flex items-center gap-2 rounded-full bg-stone-100 px-4 py-2 text-sm text-stone-700">
            <Icon icon="mdi:shield-check" className="text-lg" />
            <span>Security-first architecture</span>
          </div>
          <h1 className="mb-6 font-mono text-4xl text-stone-700 sm:text-5xl lg:text-6xl">
            Your data security
            <br />
            is our priority
          </h1>
          <p className="mx-auto max-w-3xl text-lg leading-relaxed text-neutral-600 sm:text-xl">
            At Char, we believe your meeting conversations are among your most
            sensitive data. That's why we've built security into every layer of
            our architecture, not as an afterthought, but as a foundational
            principle.
          </p>
        </header>
      </div>
    </div>
  );
}

function SecurityPrinciplesSection() {
  const principles = [
    {
      icon: "mdi:laptop",
      title: "Local-first workflows",
      description:
        "Core notes, recordings, and local-model workflows stay on your device. Cloud transcription, sync, and managed AI are opt-in.",
    },
    {
      icon: "mdi:lock",
      title: "OS-level protection",
      description:
        "Char stores core data as local files on disk. Protection comes from your operating system, file permissions, and full-disk encryption such as FileVault or LUKS.",
    },
    {
      icon: "mdi:eye-off",
      title: "Clear data flow",
      description:
        "You choose local models, BYOK providers, or managed services. The docs spell out what is sent for each path.",
    },
    {
      icon: "mdi:source-branch",
      title: "Open source transparency",
      description:
        "Every line of code is publicly auditable. Security researchers and privacy advocates can verify our claims by inspecting the source.",
    },
  ];

  return (
    <section className="px-6 py-12 lg:py-16">
      <h2 className="mb-4 text-left font-mono text-3xl text-stone-700">
        Security principles we live by
      </h2>
      <p className="mx-auto mb-12 max-w-2xl text-left text-neutral-600">
        These are product decisions and documented tradeoffs, not vague
        promises.
      </p>
      <div className="mx-auto grid max-w-4xl gap-8 md:grid-cols-2">
        {principles.map((principle, index) => (
          <div
            key={index}
            className="rounded-lg border border-neutral-200 bg-white p-6"
          >
            <Icon
              icon={principle.icon}
              className="mb-4 text-3xl text-stone-700"
            />
            <h3 className="mb-2 font-mono text-xl text-stone-700">
              {principle.title}
            </h3>
            <p className="text-neutral-600">{principle.description}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

function LocalFirstSection() {
  return (
    <section className="bg-stone-50/30 px-6 py-12 lg:py-16">
      <div className="mx-auto max-w-4xl">
        <div className="mb-12 text-left">
          <Icon
            icon="mdi:desktop-classic"
            className="mb-4 text-5xl text-stone-700"
          />
          <h2 className="mb-4 font-mono text-3xl text-stone-700">
            Local-first means secure by default
          </h2>
          <p className="mx-auto max-w-2xl text-neutral-600">
            Unlike cloud-only tools, Char can keep notes, recordings, and
            local-model workflows on your machine. If you enable cloud features,
            that data flow is explicit.
          </p>
        </div>

        <div className="grid gap-6 md:grid-cols-3">
          <div className="p-6 text-left">
            <Icon
              icon="mdi:microphone"
              className="mx-auto mb-4 text-4xl text-stone-700"
            />
            <h3 className="mb-2 font-medium text-stone-700">
              Audio stays local by default
            </h3>
            <p className="text-sm text-neutral-600">
              Recorded audio files stay on your device unless you explicitly
              choose a cloud transcription or sync feature.
            </p>
          </div>
          <div className="p-6 text-left">
            <Icon
              icon="mdi:brain"
              className="mx-auto mb-4 text-4xl text-stone-700"
            />
            <h3 className="mb-2 font-medium text-stone-700">
              Local AI is available
            </h3>
            <p className="text-sm text-neutral-600">
              You can transcribe and run AI locally with supported models, or
              switch to a cloud provider when you want managed services.
            </p>
          </div>
          <div className="p-6 text-left">
            <Icon
              icon="mdi:database"
              className="mx-auto mb-4 text-4xl text-stone-700"
            />
            <h3 className="mb-2 font-medium text-stone-700">
              File-based storage
            </h3>
            <p className="text-sm text-neutral-600">
              Core data is stored as local Markdown and JSON files on disk, not
              locked in a hosted database.
            </p>
          </div>
        </div>

        <div className="mt-12 rounded-lg border border-neutral-200 bg-white p-8">
          <div className="flex items-start gap-4">
            <Icon
              icon="mdi:wifi-off"
              className="shrink-0 text-3xl text-stone-700"
            />
            <div>
              <h3 className="mb-3 font-mono text-xl text-stone-700">
                Works offline with local models
              </h3>
              <p className="text-neutral-600">
                If you use local STT and local LLMs, Char works without an
                internet connection. Cloud transcription, sync, and managed AI
                still require network access.
              </p>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function EncryptionSection() {
  return (
    <section className="px-6 py-12 lg:py-16">
      <div className="mx-auto max-w-4xl">
        <div className="mb-12 text-left">
          <Icon icon="mdi:lock" className="mb-4 text-5xl text-stone-700" />
          <h2 className="mb-4 font-mono text-3xl text-stone-700">
            Protection and transport
          </h2>
          <p className="mx-auto max-w-2xl text-neutral-600">
            Today Char relies on operating-system protections for local files
            and encrypted transport for network requests.
          </p>
        </div>

        <div className="flex flex-col gap-6">
          <div className="rounded-lg border border-neutral-200 bg-white p-6">
            <div className="flex items-start gap-4">
              <Icon
                icon="mdi:harddisk"
                className="mt-1 shrink-0 text-2xl text-stone-700"
              />
              <div>
                <h3 className="mb-2 font-medium text-stone-700">
                  Local data protection
                </h3>
                <p className="text-neutral-600">
                  Char does not currently add its own encryption layer to local
                  files. Protection comes from your OS account permissions and
                  any full-disk encryption you enable, such as FileVault or
                  LUKS.
                </p>
              </div>
            </div>
          </div>

          <div className="rounded-lg border border-neutral-200 bg-white p-6">
            <div className="flex items-start gap-4">
              <Icon
                icon="mdi:swap-horizontal"
                className="mt-1 shrink-0 text-2xl text-stone-700"
              />
              <div>
                <h3 className="mb-2 font-medium text-stone-700">
                  Encryption in transit
                </h3>
                <p className="text-neutral-600">
                  When you use cloud features, requests are sent over encrypted
                  connections to the configured Char service or provider.
                </p>
              </div>
            </div>
          </div>

          <div className="rounded-lg border border-neutral-200 bg-white p-6">
            <div className="flex items-start gap-4">
              <Icon
                icon="mdi:key"
                className="mt-1 shrink-0 text-2xl text-stone-700"
              />
              <div>
                <h3 className="mb-2 font-medium text-stone-700">
                  Encryption roadmap
                </h3>
                <p className="text-neutral-600">
                  We are investigating an additional encryption layer for sync
                  and other cloud workflows, but we do not present that as
                  shipped today.
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function OpenSourceSecuritySection() {
  return (
    <section className="bg-stone-50/30 px-6 py-12 lg:py-16">
      <div className="mx-auto max-w-4xl">
        <div className="mb-12 text-left">
          <Icon
            icon="mdi:code-braces"
            className="mb-4 text-5xl text-stone-700"
          />
          <h2 className="mb-4 font-mono text-3xl text-stone-700">
            Security through transparency
          </h2>
          <p className="mx-auto max-w-2xl text-neutral-600">
            Open source isn't just about collaboration. It's about trust. When
            you can see exactly how your data is handled, you don't have to take
            our word for it.
          </p>
        </div>

        <div className="grid gap-8 md:grid-cols-2">
          <div className="rounded-lg border border-neutral-200 bg-white p-6">
            <Icon icon="mdi:magnify" className="mb-4 text-3xl text-stone-700" />
            <h3 className="mb-2 font-mono text-xl text-stone-700">
              Fully auditable
            </h3>
            <p className="text-neutral-600">
              Every function, every data flow, every security measure is visible
              in our public repository. Security researchers can audit our code
              and report vulnerabilities through our responsible disclosure
              program.
            </p>
          </div>
          <div className="rounded-lg border border-neutral-200 bg-white p-6">
            <Icon
              icon="mdi:account-group"
              className="mb-4 text-3xl text-stone-700"
            />
            <h3 className="mb-2 font-mono text-xl text-stone-700">
              Community reviewed
            </h3>
            <p className="text-neutral-600">
              Thousands of developers have reviewed our codebase. Bugs and
              security issues are caught faster when many eyes are watching.
              This collective vigilance makes Char more secure than any
              closed-source alternative.
            </p>
          </div>
        </div>

        <div className="mt-8 text-left">
          <a
            href="https://github.com/fastrepl/char"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 font-medium text-stone-700 hover:text-stone-800"
          >
            <Icon icon="mdi:github" className="text-lg" />
            Review our security implementation on GitHub
            <Icon icon="mdi:arrow-right" className="text-lg" />
          </a>
        </div>
      </div>
    </section>
  );
}

function EnterpriseSecuritySection() {
  const features = [
    {
      icon: "mdi:office-building",
      title: "Self-hosted deployment",
      description:
        "Deploy Char on your own infrastructure for complete control over your data and security policies.",
    },
    {
      icon: "mdi:shield-account",
      title: "SSO integration",
      description:
        "Integrate with your existing identity provider for seamless and secure authentication.",
    },
    {
      icon: "mdi:clipboard-check",
      title: "Compliance ready",
      description:
        "Built to support GDPR, HIPAA, and SOC 2 compliance requirements with comprehensive audit logging.",
    },
    {
      icon: "mdi:account-lock",
      title: "Access controls",
      description:
        "Granular role-based access controls let you define exactly who can access what data.",
    },
  ];

  return (
    <section className="px-6 py-12 lg:py-16">
      <div className="mx-auto max-w-4xl">
        <div className="mb-12 text-left">
          <h2 className="mb-4 font-mono text-3xl text-stone-700">
            Enterprise-ready security
          </h2>
          <p className="mx-auto max-w-2xl text-neutral-600">
            For organizations with advanced security requirements, Char offers
            enterprise features that meet the most demanding standards.
          </p>
        </div>

        <div className="grid gap-6 sm:grid-cols-2">
          {features.map((feature, index) => (
            <div
              key={index}
              className="rounded-lg border border-neutral-200 bg-white p-6"
            >
              <Icon
                icon={feature.icon}
                className="mb-3 text-2xl text-stone-700"
              />
              <h3 className="mb-2 font-medium text-stone-700">
                {feature.title}
              </h3>
              <p className="text-sm text-neutral-600">{feature.description}</p>
            </div>
          ))}
        </div>

        <div className="mt-8 text-left">
          <Link
            to="/enterprise/"
            className="inline-flex items-center gap-2 font-medium text-stone-700 hover:text-stone-800"
          >
            Learn more about enterprise features
            <Icon icon="mdi:arrow-right" className="text-lg" />
          </Link>
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
          icon="mdi:shield-check"
          className="mx-auto mb-6 text-5xl text-stone-700"
        />
        <h2 className="mb-4 font-mono text-3xl text-stone-700">
          Ready to take control of your meeting data?
        </h2>
        <p className="mb-8 text-neutral-600">
          Join thousands of professionals who trust Char to keep their
          conversations secure and private.
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
            to="/privacy/"
            className={cn([
              "inline-flex items-center justify-center rounded-full px-8 py-3 text-base font-medium",
              "border border-neutral-300 text-stone-700",
              "transition-colors hover:bg-stone-50",
            ])}
          >
            Learn about privacy
          </Link>
        </div>
      </div>
    </section>
  );
}
