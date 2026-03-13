import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useRef } from "react";

import { cn } from "@hypr/utils";

import { GithubStars } from "@/components/github-stars";
import { SlashSeparator } from "@/components/slash-separator";
import { CTASection } from "@/routes/_view/index";

export const Route = createFileRoute("/_view/product/self-hosting")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Self-Hosting - Char" },
      {
        name: "description",
        content:
          "Deploy Char on your own infrastructure. Complete control over your meeting data with on-premises deployment, air-gapped environments, and full data sovereignty.",
      },
      { name: "robots", content: "noindex, nofollow" },
      { property: "og:title", content: "Self-Hosting - Char" },
      {
        property: "og:description",
        content:
          "Run Char entirely on your own servers. Enterprise-grade meeting transcription with complete infrastructure control, compliance-ready deployment, and zero external dependencies.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://hyprnote.com/product/self-hosting",
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
        <WhySelfHostSection />
        <SlashSeparator />
        <ComparisonSection />
        <SlashSeparator />
        <DeploymentOptionsSection />
        <SlashSeparator />
        <WhatYouCanHostSection />
        <SlashSeparator />
        <EnterpriseSection />
        <SlashSeparator />
        <OpenSourceSection />
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
            Run Char on
            <br />
            your infrastructure
          </h1>
          <p className="mx-auto max-w-3xl text-lg text-neutral-600 sm:text-xl">
            Deploy Char entirely on your own servers. Complete control over your
            meeting data with on-premises deployment, air-gapped environments,
            and full data sovereignty.
          </p>
        </div>
        <div className="flex flex-col items-center gap-4 pt-6 sm:flex-row">
          <GithubStars />
          <Link
            to="/opensource/"
            className={cn([
              "flex h-12 items-center justify-center rounded-full px-6 text-base font-medium",
              "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900",
              "shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]",
            ])}
          >
            Our open source manifesto
          </Link>
        </div>
      </div>
    </section>
  );
}

function WhySelfHostSection() {
  return (
    <section>
      <div className="border-b border-neutral-100 text-center">
        <p className="py-6 font-serif font-medium tracking-wide text-neutral-600 uppercase">
          Why self-host
        </p>
      </div>
      <div className="grid md:grid-cols-2">
        <div className="border-r border-b border-neutral-100 p-8 md:border-b-0">
          <Icon
            icon="mdi:server-security"
            className="mb-4 text-3xl text-stone-700"
          />
          <h3 className="mb-2 font-serif text-xl text-stone-700">
            Complete data sovereignty
          </h3>
          <p className="text-neutral-600">
            Your meeting recordings, transcripts, and AI-generated summaries
            stay within your network perimeter. No data ever leaves your
            infrastructure.
          </p>
        </div>
        <div className="border-b border-neutral-100 p-8 md:border-b-0">
          <Icon
            icon="mdi:shield-check"
            className="mb-4 text-3xl text-stone-700"
          />
          <h3 className="mb-2 font-serif text-xl text-stone-700">
            Compliance ready
          </h3>
          <p className="text-neutral-600">
            Meet HIPAA, GDPR, SOC 2, and other regulatory requirements with
            on-premises deployment. Simplify audits with complete infrastructure
            control.
          </p>
        </div>
        <div className="border-r border-neutral-100 p-8">
          <Icon icon="mdi:lan" className="mb-4 text-3xl text-stone-700" />
          <h3 className="mb-2 font-serif text-xl text-stone-700">
            Air-gapped deployment
          </h3>
          <p className="text-neutral-600">
            Run Char in completely isolated environments with zero internet
            connectivity. Perfect for defense, government, and high-security
            organizations.
          </p>
        </div>
        <div className="p-8">
          <Icon
            icon="mdi:tune-vertical"
            className="mb-4 text-3xl text-stone-700"
          />
          <h3 className="mb-2 font-serif text-xl text-stone-700">
            Full customization
          </h3>
          <p className="text-neutral-600">
            Modify any component to fit your workflow. Integrate with internal
            systems, customize AI models, and adapt the interface to your needs.
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
          Self-hosted vs. Cloud
        </p>
      </div>
      <div className="grid md:grid-cols-2">
        <div className="border-r border-neutral-100 p-8">
          <div className="mb-6 flex items-center gap-2">
            <Icon icon="mdi:cloud" className="text-2xl text-neutral-400" />
            <h3 className="font-serif text-lg text-neutral-700">
              Cloud-hosted Solutions
            </h3>
          </div>
          <ul className="flex flex-col gap-4 text-neutral-600">
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:close"
                className="mt-0.5 shrink-0 text-neutral-400"
              />
              <span>Data stored on vendor servers</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:close"
                className="mt-0.5 shrink-0 text-neutral-400"
              />
              <span>Limited compliance control</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:close"
                className="mt-0.5 shrink-0 text-neutral-400"
              />
              <span>Vendor lock-in risks</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:close"
                className="mt-0.5 shrink-0 text-neutral-400"
              />
              <span>Internet dependency</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:close"
                className="mt-0.5 shrink-0 text-neutral-400"
              />
              <span>Third-party data access risks</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:close"
                className="mt-0.5 shrink-0 text-neutral-400"
              />
              <span>Limited customization options</span>
            </li>
          </ul>
        </div>
        <div className="bg-green-50/50 p-8">
          <div className="mb-6 flex items-center gap-2">
            <Icon icon="mdi:server" className="text-2xl text-green-600" />
            <h3 className="font-serif text-lg text-green-900">
              Char Self-hosted
            </h3>
          </div>
          <ul className="flex flex-col gap-4 text-green-900">
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:check"
                className="mt-0.5 shrink-0 text-green-600"
              />
              <span>Data stays on your servers</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:check"
                className="mt-0.5 shrink-0 text-green-600"
              />
              <span>Full compliance control</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:check"
                className="mt-0.5 shrink-0 text-green-600"
              />
              <span>No vendor dependencies</span>
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
              <span>Zero external data access</span>
            </li>
            <li className="flex items-start gap-3">
              <Icon
                icon="mdi:check"
                className="mt-0.5 shrink-0 text-green-600"
              />
              <span>Fully customizable codebase</span>
            </li>
          </ul>
        </div>
      </div>
    </section>
  );
}

function DeploymentOptionsSection() {
  return (
    <section>
      <div className="border-b border-neutral-100 text-center">
        <p className="py-6 font-serif font-medium tracking-wide text-neutral-600 uppercase">
          Deployment options
        </p>
      </div>
      <div className="grid sm:grid-cols-2 lg:grid-cols-3">
        <div className="border-r border-b border-neutral-100 p-6">
          <Icon icon="mdi:docker" className="mb-3 text-2xl text-stone-700" />
          <h3 className="mb-2 font-medium text-stone-700">Docker containers</h3>
          <p className="text-sm text-neutral-600">
            Deploy with Docker Compose for quick setup. Ideal for teams wanting
            containerized infrastructure.
          </p>
        </div>
        <div className="border-r border-b border-neutral-100 p-6 lg:border-r">
          <Icon
            icon="mdi:kubernetes"
            className="mb-3 text-2xl text-stone-700"
          />
          <h3 className="mb-2 font-medium text-stone-700">Kubernetes</h3>
          <p className="text-sm text-neutral-600">
            Scale across clusters with Helm charts. Built for enterprise
            orchestration and high availability.
          </p>
        </div>
        <div className="border-b border-neutral-100 p-6 sm:border-r lg:border-r-0">
          <Icon icon="mdi:server" className="mb-3 text-2xl text-stone-700" />
          <h3 className="mb-2 font-medium text-stone-700">Bare metal</h3>
          <p className="text-sm text-neutral-600">
            Install directly on your servers for maximum performance and
            complete hardware control.
          </p>
        </div>
        <div className="border-r border-b border-neutral-100 p-6 sm:border-b-0 lg:border-b-0">
          <Icon
            icon="mdi:cloud-lock"
            className="mb-3 text-2xl text-stone-700"
          />
          <h3 className="mb-2 font-medium text-stone-700">Private cloud</h3>
          <p className="text-sm text-neutral-600">
            Deploy on AWS, GCP, or Azure within your VPC. Your cloud, your
            rules, your data.
          </p>
        </div>
        <div className="border-r border-neutral-100 p-6">
          <Icon
            icon="mdi:lan-disconnect"
            className="mb-3 text-2xl text-stone-700"
          />
          <h3 className="mb-2 font-medium text-stone-700">Air-gapped</h3>
          <p className="text-sm text-neutral-600">
            Fully isolated deployment with no network connectivity. All
            dependencies bundled offline.
          </p>
        </div>
        <div className="p-6">
          <Icon
            icon="mdi:office-building"
            className="mb-3 text-2xl text-stone-700"
          />
          <h3 className="mb-2 font-medium text-stone-700">On-premises</h3>
          <p className="text-sm text-neutral-600">
            Run in your own data center with dedicated hardware. Maximum
            security and performance.
          </p>
        </div>
      </div>
    </section>
  );
}

function WhatYouCanHostSection() {
  return (
    <section>
      <div className="border-b border-neutral-100 text-center">
        <p className="py-6 font-serif font-medium tracking-wide text-neutral-600 uppercase">
          What you can self-host
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
              Transcription server
            </h3>
            <p className="text-neutral-600">
              Run Whisper models on your own GPU servers. Process unlimited
              audio without external API calls. Supports multiple model sizes
              from tiny to large-turbo.
            </p>
          </div>
        </div>
        <div className="flex items-start gap-4 p-8">
          <Icon icon="mdi:brain" className="shrink-0 text-3xl text-stone-700" />
          <div>
            <h3 className="mb-2 font-serif text-xl text-stone-700">
              LLM inference
            </h3>
            <p className="text-neutral-600">
              Deploy HyperLLM or bring your own models. Run summarization,
              extraction, and analysis entirely within your network. Compatible
              with Ollama and vLLM.
            </p>
          </div>
        </div>
        <div className="flex items-start gap-4 p-8">
          <Icon
            icon="mdi:database"
            className="shrink-0 text-3xl text-stone-700"
          />
          <div>
            <h3 className="mb-2 font-serif text-xl text-stone-700">
              Data storage
            </h3>
            <p className="text-neutral-600">
              Store all meeting data in your own database. SQLite for
              single-node, PostgreSQL for distributed deployments. Full backup
              and migration control.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}

function EnterpriseSection() {
  return (
    <section>
      <div className="border-b border-neutral-100 text-center">
        <p className="py-6 font-serif font-medium tracking-wide text-neutral-600 uppercase">
          Built for enterprise
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
              HIPAA & SOC 2 ready
            </h3>
            <p className="text-neutral-600">
              Self-hosting simplifies compliance by keeping all data within your
              controlled environment. No third-party processors to audit.
            </p>
          </div>
        </div>
        <div className="flex gap-4 border-b border-neutral-100 p-8 md:border-b-0">
          <Icon
            icon="mdi:account-group"
            className="shrink-0 text-3xl text-blue-600"
          />
          <div>
            <h3 className="mb-2 font-serif text-lg text-stone-700">
              SSO & LDAP integration
            </h3>
            <p className="text-neutral-600">
              Connect to your existing identity provider. Support for SAML,
              OIDC, and Active Directory out of the box.
            </p>
          </div>
        </div>
        <div className="flex gap-4 border-r border-neutral-100 p-8">
          <Icon
            icon="mdi:chart-line"
            className="shrink-0 text-3xl text-purple-600"
          />
          <div>
            <h3 className="mb-2 font-serif text-lg text-stone-700">
              Usage analytics
            </h3>
            <p className="text-neutral-600">
              Monitor adoption and usage across your organization with built-in
              analytics. All data stays internal.
            </p>
          </div>
        </div>
        <div className="flex gap-4 p-8">
          <Icon
            icon="mdi:headset"
            className="shrink-0 text-3xl text-orange-600"
          />
          <div>
            <h3 className="mb-2 font-serif text-lg text-stone-700">
              Dedicated support
            </h3>
            <p className="text-neutral-600">
              Enterprise customers get priority support, deployment assistance,
              and custom development options.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}

function OpenSourceSection() {
  return (
    <section>
      <div className="border-b border-neutral-100 text-center">
        <p className="py-6 font-serif font-medium tracking-wide text-neutral-600 uppercase">
          Open source foundation
        </p>
      </div>
      <div className="flex items-start gap-4 p-8">
        <Icon icon="mdi:github" className="shrink-0 text-4xl text-stone-700" />
        <div>
          <h3 className="mb-3 font-serif text-xl text-stone-700">
            Fully auditable codebase
          </h3>
          <p className="mb-4 text-neutral-600">
            Char is open source under GPL-3.0. Inspect every line of code,
            verify security practices, and customize to your needs. No black
            boxes, no hidden data collection.
          </p>
          <a
            href="https://github.com/fastrepl/char"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 font-medium text-stone-700 hover:text-stone-800"
          >
            <Icon icon="mdi:github" className="text-xl" />
            View on GitHub
            <Icon icon="mdi:arrow-right" className="text-lg" />
          </a>
        </div>
      </div>
    </section>
  );
}
