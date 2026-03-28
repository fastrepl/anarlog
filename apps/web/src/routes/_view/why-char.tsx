import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useRef } from "react";

import { CTASection } from "@/components/cta-section";
import { Image } from "@/components/image";

export const Route = createFileRoute("/_view/why-char")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Why Char - AI Meeting Notes You Actually Own" },
      {
        name: "description",
        content:
          "Your meeting notes should be files on your computer, not rows in someone else's database. Plain Markdown files, AI providers you can switch, no bots, no lock-in.",
      },
      { property: "og:title", content: "Why Char" },
      {
        property: "og:description",
        content:
          "Most AI note-takers lock your data in their database. We thought that was bullshit. So we built Char differently.",
      },
      { property: "og:type", content: "website" },
      { property: "og:url", content: "https://char.com/why-char" },
      { name: "twitter:card", content: "summary_large_image" },
      { name: "twitter:title", content: "Why Char" },
      {
        name: "twitter:description",
        content:
          "Your meeting notes should be files on your computer, not rows in someone else's database.",
      },
    ],
  }),
});

function Component() {
  const heroInputRef = useRef<HTMLInputElement>(null);

  return (
    <div className="min-h-screen">
      <div className="mx-auto">
        <HeroSection />
        <WhyWereDifferentSection />

        <WhoThisIsForSection />

        <WhatWereBuildingTowardSection />

        <HereForTheLongHaulSection />
        <CTASection heroInputRef={heroInputRef} />
      </div>
    </div>
  );
}

function HeroSection() {
  return (
    <div className="flex w-full min-w-0 flex-col text-left">
      <section className="laptop:px-4 isolate flex w-full min-w-0 overflow-visible pt-10 text-left">
        <div className="border-brand-bright relative z-10 flex w-full min-w-0 flex-col content-between rounded-lg border md:min-h-[60vh] md:flex-row">
          <div className="flex flex-col justify-center pt-12 pr-8 pb-12 pl-12">
            <div className="flex flex-col gap-6">
              <h1 className="text-color text-2xl break-words sm:text-6xl">
                Why Char exists
              </h1>
              <p className="text-fg-muted text-2xl leading-relaxed break-words">
                Most AI note-takers lock your data in their database, force you
                to use their AI stack, and make you lose everything if you
                leave.
              </p>
              <p className="text-fg-muted text-2xl leading-relaxed font-medium break-words">
                We thought that was bullshit.
              </p>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}

const differentiators = [
  {
    title: "Plain Markdown files",
    description: "Not proprietary databases—files you own that work in any app",
    icon: "mdi:file-document-outline",
  },
  {
    title: "No meeting bots",
    description:
      "System audio capture works everywhere: Zoom, Teams, phone calls, in-person",
    icon: "mdi:microphone-off",
  },
  {
    title: "Choose your AI",
    description:
      "Managed service, bring your own key, or run fully local models",
    icon: "mdi:brain",
  },
  {
    title: "Open source",
    description: "Public code that security teams can audit and verify",
    icon: "mdi:github",
  },
  {
    title: "Zero lock-in",
    description:
      "Export anytime, switch providers anytime, or just stop using us",
    icon: "mdi:lock-open-outline",
  },
];

function WhyWereDifferentSection() {
  return (
    <section className="px-4 py-16 lg:py-24">
      <div className="mx-auto">
        <h2 className="text-color mb-12 text-left font-mono text-3xl sm:text-4xl">
          So we built Char to give you back control.
        </h2>

        <div className="flex flex-col gap-6 md:flex-row md:flex-nowrap">
          {differentiators.map((item) => (
            <div
              key={item.title}
              className="border-color-brand bg-surface flex min-w-0 flex-1 flex-col gap-4 rounded-lg border p-6"
            >
              <div className="w-fit p-2">
                <Icon icon={item.icon} className="text-fg-muted text-2xl" />
              </div>
              <div className="min-w-0">
                <h3 className="text-color mb-1 font-semibold">{item.title}</h3>
                <p className="text-fg-muted text-sm">{item.description}</p>
              </div>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

const audiences = [
  {
    title: "Your company banned Otter/ChatGPT/Granola",
    description:
      "Your IT team can audit the open-source code. Files stay on your device. You can use whichever AI provider your company already approved or run everything locally.",
    icon: "mdi:shield-check-outline",
  },
  {
    title: "You're deep into Obsidian/Logseq/PKM systems",
    description:
      "You've spent years building a knowledge vault in Markdown. Your meeting notes shouldn't live in a separate app that doesn't integrate with anything.",
    icon: "mdi:note-multiple-outline",
  },
  {
    title: "You already pay for OpenAI/Anthropic API credits",
    description:
      "Why pay markup on top of API costs you already have? Bring your own key and use the credits you're already buying.",
    icon: "mdi:key-outline",
  },
  {
    title: "You're an open-source advocate who self-hosts everything",
    description:
      "You run Nextcloud, care about FOSS, and need to verify no data leaves your infrastructure. Char lets you audit the code and run everything locally.",
    icon: "mdi:server-outline",
  },
  {
    title: "You just want a simple notepad that works",
    description:
      "You don't care about the philosophy. You want to take notes during calls without thinking about it. Char does that.",
    icon: "mdi:notebook-outline",
  },
];

function WhoThisIsForSection() {
  return (
    <section className="px-4 py-16 lg:py-24">
      <div className="mx-auto">
        <h2 className="text-color mb-8 text-left font-mono text-3xl sm:text-4xl">
          Char's for you, if
        </h2>

        <div className="flex flex-col gap-8">
          {audiences.map((item) => (
            <div
              key={item.title}
              className="border-color-brand surface flex gap-4 rounded-lg border p-6"
            >
              <div className="flex size-10 h-fit shrink-0 items-center justify-center rounded-lg border border-neutral-100 bg-white p-2">
                <Icon icon={item.icon} className="text-fg-muted text-2xl" />
              </div>
              <div>
                <h3 className="text-color mb-4 font-semibold">{item.title}</h3>
                <p className="text-fg-muted leading-relaxed">
                  {item.description}
                </p>
              </div>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function WhatWereBuildingTowardSection() {
  return (
    <section className="laptop:px-0 bg-[linear-gradient(to_right,#f5f5f5_1px,transparent_1px),linear-gradient(to_bottom,#f5f5f5_1px,transparent_1px)] bg-size-[24px_24px] bg-position-[12px_12px,12px_12px] px-4 py-16">
      <div className="mx-auto">
        <div className="border-color-brand surface rounded-lg border p-4">
          <div
            className="border-color-brand bg-surface rounded-xs border p-8 sm:p-12"
            style={{
              backgroundImage: "url(/api/images/texture/paper.png)",
            }}
          >
            <h2 className="text-color mb-4 font-mono text-2xl sm:text-3xl">
              What we're building toward
            </h2>

            <div className="text-color flex max-w-2xl flex-col gap-4 leading-relaxed">
              <p>
                We're not betting on GPT-5 or Claude Opus 7 or whatever comes
                next.
              </p>

              <p>We're betting on files.</p>

              <p>
                Files outlive apps. Files work with every tool. Files don't
                disappear when a startup shuts down.
              </p>

              <p>
                AI providers will come and go. SaaS platforms will rise and
                fall. But Markdown files from 2006 still open perfectly in 2026.
              </p>

              <p>
                That's the foundation. Everything else is just software on top.
              </p>
            </div>

            <div className="mt-12 mb-4 flex gap-2">
              <Image
                src="/api/images/team/john.png"
                alt="John Jeong"
                width={32}
                height={32}
                className="rounded-full border border-neutral-200 object-cover"
              />
              <Image
                src="/api/images/team/yujong.png"
                alt="Yujong Lee"
                width={32}
                height={32}
                className="rounded-full border border-neutral-200 object-cover"
              />
            </div>

            <div className="flex flex-col gap-4">
              <div>
                <p className="text-fg-muted font-mono text-base font-medium italic">
                  Char
                </p>
                <p className="text-fg-muted text-sm">John Jeong, Yujong Lee</p>
              </div>

              <div>
                <Image
                  src="/char-signature.svg"
                  alt="Char Signature"
                  width={124}
                  height={60}
                  layout="constrained"
                  className="object-contain opacity-80"
                />
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

const commitments = [
  "No auto-renewal traps",
  "No annual price increases",
  "No forcing you onto annual contracts",
  'No hiding features behind "contact sales"',
  "No meeting bots that make your coworkers uncomfortable",
];

function HereForTheLongHaulSection() {
  return (
    <section className="px-4 py-16 lg:py-24">
      <div className="mx-auto max-w-3xl">
        <h2 className="text-color mb-8 text-left font-mono text-3xl sm:text-4xl">
          Here for the long haul
        </h2>

        <div className="text-color flex flex-col gap-6 leading-relaxed">
          <p>
            This isn't a bait-and-switch. We're not looking to get acquired and
            cash out.
          </p>

          <p>
            <span className="text-color font-semibold">
              We're building the company we want to work for
            </span>
            —one that treats users the way we'd want to be treated.
          </p>

          <p>That means:</p>

          <ul className="flex flex-col gap-3">
            {commitments.map((commitment) => (
              <li key={commitment} className="flex items-center gap-3">
                <div className="flex size-6 shrink-0 items-center justify-center rounded-full bg-stone-100 p-1">
                  <Icon icon="mdi:check" className="text-fg-muted text-lg" />
                </div>
                <span>{commitment}</span>
              </li>
            ))}
          </ul>

          <p className="mt-4">
            If that sounds like the kind of company you want to support,{" "}
            <Link
              to="/"
              hash="hero"
              className="text-color font-semibold decoration-dotted hover:underline"
            >
              download Char and try it
            </Link>
            .
          </p>

          <p className="text-color text-lg font-medium">
            If we screw this up, you can export everything and walk away. That's
            the deal.
          </p>
        </div>
      </div>
    </section>
  );
}
