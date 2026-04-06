import { Icon } from "@iconify-icon/react";
import { createFileRoute } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";
import { Fragment, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import { CTASection } from "@/components/cta-section";
import { DownloadButton } from "@/components/download-button";
import { RotatingAvatarGrid } from "@/components/github-open-source";
import { GithubStars } from "@/components/github-stars";
import { Image } from "@/components/image";
import {
  GITHUB_LAST_SEEN_FORKS,
  GITHUB_LAST_SEEN_STARS,
  useGitHubStargazers,
  useGitHubStats,
} from "@/queries";

export const Route = createFileRoute("/_view/opensource")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Open Source - Char" },
      {
        name: "description",
        content:
          "Char is fully open source under GPL-3.0. Inspect every line of code, contribute to development, and build on a transparent foundation. No black boxes, no hidden data collection.",
      },
      { property: "og:title", content: "Open Source - Char" },
      {
        property: "og:description",
        content:
          "AI-powered meeting notes built in the open. Fully auditable codebase, community-driven development, and complete transparency. Join thousands of developers building the future of private meeting notes.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/opensource",
      },
      { name: "twitter:card", content: "summary_large_image" },
      { name: "twitter:title", content: "Open Source - Char" },
      {
        name: "twitter:description",
        content:
          "AI-powered meeting notes built in the open. Fully auditable codebase and community-driven development.",
      },
      {
        name: "keywords",
        content:
          "open source, meeting notes, AI transcription, privacy, GPL-3.0, Rust, Tauri, local AI, whisper, llm",
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
        <LetterSection />
        <TechStackSection />
        <SponsorsSection />
        <ProgressSection />
        <JoinMovementSection />
        <CTASection heroInputRef={heroInputRef} />
      </div>
    </div>
  );
}

function HeroSection() {
  const { data: stargazers = [] } = useGitHubStargazers();

  return (
    <div className="isolate flex w-full overflow-visible pt-10 text-left">
      <div className="border-brand-bright surface relative z-10 flex w-full flex-col rounded-lg border px-6 pt-8 pb-8 md:px-12 md:pt-12 md:pb-12">
        <div className="flex flex-col gap-2">
          <h1
            className="text-color break-words"
            style={{
              fontSize: "clamp(1.5rem, 0.75rem + 3.2vw, 3.75rem)",
            }}
          >
            Built in the open,
            <br />
            for everyone
          </h1>
          <p className="text-fg-muted max-w-3xl text-base leading-relaxed break-words sm:text-xl">
            Char is fully open source under GPL-3.0. Every line of code is
            auditable, every decision is transparent, and every user has the
            freedom to inspect, modify, and contribute.
          </p>
          <div className="mt-4 flex w-full flex-col items-stretch gap-4 lg:flex-row lg:items-start">
            <DownloadButton />
            <GithubStars />
          </div>
        </div>
        {stargazers.length > 0 ? (
          <RotatingAvatarGrid profiles={stargazers} rows={2} />
        ) : null}
      </div>
    </div>
  );
}

function LetterSection() {
  return (
    <section className="px-6 py-16 lg:py-24">
      <div className="mx-auto max-w-3xl">
        <div className="mb-8 text-left">
          <span className="text-fg-muted font-mono text-sm font-medium tracking-widest uppercase">
            A letter from our team
          </span>
        </div>

        <article>
          <h1 className="text-color mb-12 text-left font-mono text-3xl sm:text-4xl lg:text-5xl">
            Why Open Source is Inevitable
            <br />
            in the Age of AI
          </h1>

          <div className="text-fg-muted flex flex-col gap-6 leading-relaxed">
            <p className="text-lg">Hey friends,</p>

            <p>
              We're watching software change faster than any of us expected. AI
              isn't a concept anymore. It's in your meetings, it's inside your
              documents, and it has context on things that used to live only in
              your mind.
            </p>

            <p>
              When software listens to you, when it transcribes you, when it
              summarizes your thinking, trust can't just be a marketing claim.
            </p>

            <p>That's why open source is not a nice-to-have. It's mandatory.</p>

            <p>
              If an AI tool captures your voice, your discussions, your
              strategy, you should be able to see exactly what it does with that
              information. Not a PDF saying "we care about privacy." Not a
              privacy policy written by lawyers. Actual code.
            </p>

            <p>
              Closed-source AI tools say "trust us." But you can't audit "trust
              us." You can't fork it, stress-test it, or guarantee your own
              compliance.
            </p>

            <p>In the age of AI, blind trust is basically an attack vector.</p>

            <p>Open source flips the power dynamic:</p>

            <ul className="flex list-disc flex-col gap-2 pl-6">
              <li>You can verify claims instead of believing them.</li>
              <li>Security researchers can inspect, not speculate.</li>
              <li>Teams can self-host, extend, or fork when needed.</li>
              <li>The product outlives the company that built it.</li>
            </ul>

            <p>That's why we built Char in the open.</p>

            <p>
              We don't want you to trust us more. We want you to need to trust
              us less. If you can inspect it, run it locally, modify it, or
              audit it, the entire idea of trust changes.
            </p>

            <p>This isn't ideology. It's durability.</p>

            <p>
              Companies die. Pricing changes. Terms change. Acquisitions happen.
              Compliance requirements evolve.
            </p>

            <p>Open source survives all of that.</p>

            <p>
              What AI is capable of today demands a different contract between
              software and the people who rely on it. That contract should be
              inspectable, forkable, and owned by its users, not hidden behind
              opaque servers.
            </p>

            <p>
              If AI ends up shaping how we work, think, and communicate, then
              the people using it deserve transparency—not promises.
            </p>

            <div className="flex flex-col gap-4">
              <div className="flex gap-2">
                <Image
                  src="/api/images/team/john.png"
                  alt="John Jeong"
                  width={32}
                  height={32}
                  className="border-color-brand rounded-full border object-cover"
                />
                <Image
                  src="/api/images/team/yujong.png"
                  alt="Yujong Lee"
                  width={32}
                  height={32}
                  className="border-color-brand rounded-full border object-cover"
                />
              </div>

              <div className="flex flex-col gap-3">
                <div>
                  <p className="text-lg">With clarity,</p>
                  <p>John Jeong, Yujong Lee</p>
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
        </article>
      </div>
    </section>
  );
}

const techStack = [
  {
    category: "Languages",
    items: [
      {
        name: "Rust",
        icon: "devicon:rust",
        description: "Core language for audio processing and local AI",
        url: "https://www.rust-lang.org/",
      },
      {
        name: "TypeScript",
        icon: "devicon:typescript",
        description: "Type-safe language for frontend development",
        url: "https://www.typescriptlang.org/",
      },
    ],
  },
  {
    category: "Desktop & UI",
    items: [
      {
        name: "Tauri",
        icon: "devicon:tauri",
        description: "Cross-platform desktop framework",
        url: "https://tauri.app/",
      },
      {
        name: "React",
        icon: "devicon:react",
        description: "UI framework for building interfaces",
        url: "https://react.dev/",
      },
      {
        name: "TanStack Start",
        imageUrl: "https://avatars.githubusercontent.com/u/72518640?s=200&v=4",
        description: "Full-stack React framework with type-safe routing",
        url: "https://tanstack.com/start",
      },
    ],
  },
  {
    category: "Build & Tooling",
    items: [
      {
        name: "Vite",
        icon: "devicon:vitejs",
        description: "Fast build tool and dev server",
        url: "https://vite.dev/",
      },
      {
        name: "Turborepo",
        icon: "vscode-icons:file-type-light-turbo",
        description: "High-performance monorepo build system",
        url: "https://turbo.build/repo",
      },
      {
        name: "pnpm",
        icon: "devicon:pnpm",
        description: "Fast, disk space efficient package manager",
        url: "https://pnpm.io/",
      },
    ],
  },
  {
    category: "AI & Data",
    items: [
      {
        name: "WhisperKit",
        imageUrl: "https://avatars.githubusercontent.com/u/150409474?s=200&v=4",
        description: "Local speech-to-text transcription",
        url: "https://github.com/argmaxinc/WhisperKit",
      },
      {
        name: "llama.cpp",
        imageUrl: "https://avatars.githubusercontent.com/u/134263123?s=200&v=4",
        description: "Local LLM inference engine",
        url: "https://github.com/ggerganov/llama.cpp",
      },
      {
        name: "TinyBase",
        imageUrl: "https://avatars.githubusercontent.com/u/96894742?s=200&v=4",
        description: "Reactive data store for local-first apps",
        url: "https://tinybase.org/",
      },
      {
        name: "TanStack Query",
        icon: "logos:react-query-icon",
        description: "Powerful data synchronization for React",
        url: "https://tanstack.com/query",
      },
    ],
  },
];

const sponsors = [
  {
    name: "Tauri",
    icon: "devicon:tauri",
    url: "https://github.com/tauri-apps",
    description: "Desktop framework",
  },
  {
    name: "MrKai77",
    imageUrl: "https://avatars.githubusercontent.com/u/68963405?v=4",
    url: "https://github.com/MrKai77",
    description: "Loop window manager",
  },
  {
    name: "James Pearce",
    imageUrl: "https://avatars.githubusercontent.com/u/90942?v=4",
    url: "https://github.com/jamesgpearce",
    description: "Open source contributor",
  },
];

function TechStackSection() {
  return (
    <section>
      <div>
        <div className="px-6 py-12 lg:py-16">
          <h2 className="text-color mb-4 text-left font-mono text-3xl">
            Our Tech Stack
          </h2>
          <p className="text-fg-muted max-w-2xl text-left">
            Built with modern, privacy-respecting technologies that run locally
            on your device.
          </p>
        </div>

        <div className="border-color-brand grid grid-cols-6 gap-4 rounded-lg border">
          {techStack.map((section) => {
            return (
              <Fragment key={section.category}>
                <div className="col-span-6 px-8 pt-8">
                  <h3 className="text-color font-mono text-xl">
                    {section.category}
                  </h3>
                </div>
                {section.items.map((tech) => {
                  return (
                    <a
                      key={tech.name}
                      href={tech.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className={cn([
                        "col-span-6 sm:col-span-3 lg:col-span-2",
                        "p-6",
                        "group transition-all",
                      ])}
                    >
                      <div className="mb-3 flex items-center gap-3">
                        {"imageUrl" in tech ? (
                          <img
                            src={tech.imageUrl}
                            alt={`${tech.name} logo`}
                            className="h-6 w-6 rounded object-cover"
                          />
                        ) : (
                          <Icon
                            icon={tech.icon}
                            className="text-color text-2xl transition-colors"
                          />
                        )}
                        <h4 className="text-color font-medium transition-colors">
                          {tech.name}
                        </h4>
                      </div>
                      <p className="text-fg-muted text-sm">
                        {tech.description}
                      </p>
                    </a>
                  );
                })}
              </Fragment>
            );
          })}
        </div>
      </div>
    </section>
  );
}

function SponsorsSection() {
  return (
    <section>
      <div>
        <div className="px-6 py-12 lg:py-16">
          <h2 className="text-color mb-4 text-left font-mono text-3xl">
            Paying It Forward
          </h2>
          <p className="text-fg-muted max-w-2xl text-left">
            We love giving back to the community that makes Char possible. As we
            grow, we hope to sponsor even more projects and creators.
          </p>
        </div>

        <div className="border-color-brand overflow-hidden rounded-lg border">
          <div className="px-8 pt-8">
            <h3 className="text-color font-mono text-xl">
              Projects We Sponsor
            </h3>
          </div>
          <div className="grid grid-cols-1 gap-4 p-8 sm:grid-cols-3">
            {sponsors.map((sponsor) => (
              <a
                key={sponsor.name}
                href={sponsor.url}
                target="_blank"
                rel="noopener noreferrer"
                className="group transition-all"
              >
                <div className="mb-3 flex items-center gap-3">
                  {"imageUrl" in sponsor ? (
                    <img
                      src={sponsor.imageUrl}
                      alt={`${sponsor.name} avatar`}
                      className="h-6 w-6 rounded-full object-cover"
                    />
                  ) : (
                    <Icon
                      icon={sponsor.icon}
                      className="text-color text-2xl transition-colors"
                    />
                  )}
                  <h4 className="text-color font-medium transition-colors">
                    {sponsor.name}
                  </h4>
                </div>
                <p className="text-fg-muted text-sm">{sponsor.description}</p>
              </a>
            ))}
          </div>
          <div className="border-color-brand surface-subtle flex flex-col gap-4 border-t px-8 py-6 lg:flex-row lg:items-center lg:justify-between">
            <div>
              <h3 className="text-color font-mono text-xl">
                We Appreciate Your Support
              </h3>
              <p className="text-fg-muted mt-2 text-sm">
                Your sponsorship keeps Char free, open source, and independent
                for everyone.
              </p>
            </div>
            <a
              href="https://github.com/sponsors/fastrepl"
              target="_blank"
              rel="noopener noreferrer"
              className={cn([
                "inline-flex shrink-0 items-center justify-center gap-2 rounded-full px-6 py-3 font-medium",
                "border-color-brand text-fg border",
                "transition-transform hover:scale-[102%] active:scale-[98%]",
              ])}
            >
              <Icon icon="mdi:heart" className="text-lg text-red-400" />
              Sponsor on GitHub
            </a>
          </div>
        </div>
      </div>
    </section>
  );
}

function ConfettiIcons({
  icon,
  imageUrl,
  color,
  count = 30,
}: {
  icon?: string;
  imageUrl?: string;
  color: string;
  count?: number;
}) {
  const icons = Array.from({ length: count }, (_, i) => ({
    id: i,
    x: Math.random() * 100,
    delay: Math.random() * 0.8,
    duration: 0.6 + Math.random() * 0.8,
    rotation: Math.random() * 720 - 360,
    scale: 0.5 + Math.random() * 1,
    xDrift: Math.random() * 60 - 30,
  }));

  return (
    <div className="pointer-events-none absolute inset-0 overflow-hidden">
      <AnimatePresence>
        {icons.map((item) => (
          <motion.div
            key={item.id}
            initial={{
              y: -30,
              x: 0,
              opacity: 0,
              rotate: 0,
              scale: item.scale,
            }}
            animate={{
              y: 150,
              x: item.xDrift,
              opacity: [0, 1, 1, 1, 0],
              rotate: item.rotation,
              scale: item.scale,
            }}
            exit={{ opacity: 0 }}
            transition={{
              duration: item.duration,
              delay: item.delay,
              ease: [0.25, 0.46, 0.45, 0.94],
            }}
            className="absolute"
            style={{ left: `${item.x}%` }}
          >
            {imageUrl ? (
              <img src={imageUrl} alt="" className="h-5 w-5 rounded" />
            ) : icon ? (
              <Icon icon={icon} className={cn(["text-xl", color])} />
            ) : null}
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}

function StatCard({
  label,
  value,
  icon,
  imageUrl,
  color,
  hasBorder,
}: {
  label: string;
  value: string;
  icon?: string;
  imageUrl?: string;
  color: string;
  hasBorder: boolean;
}) {
  const [isHovered, setIsHovered] = useState(false);

  const confettiIcon = icon === "mdi:account-group" ? "mdi:account" : icon;

  return (
    <div
      className={cn([
        "border-color-brand relative flex h-32 flex-col justify-between gap-3 p-6 text-left",
        hasBorder && "border-r",
      ])}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      {isHovered && (confettiIcon || imageUrl) && (
        <ConfettiIcons icon={confettiIcon} imageUrl={imageUrl} color={color} />
      )}
      <div className="flex flex-1 items-center justify-center">
        {imageUrl ? (
          <Image
            src={imageUrl}
            alt={label}
            width={32}
            height={32}
            className="rounded-lg object-cover"
          />
        ) : icon ? (
          <Icon icon={icon} className={cn(["text-3xl", color])} />
        ) : null}
      </div>
      <div>
        <p className="text-color text-2xl font-bold">{value}</p>
        <p className="text-fg-muted text-sm">{label}</p>
      </div>
    </div>
  );
}

function ProgressSection() {
  const { data } = useGitHubStats();
  const stars = data?.stars;
  const forks = data?.forks;

  const stats = [
    {
      label: "GitHub Stars",
      value: stars?.toLocaleString() ?? GITHUB_LAST_SEEN_STARS.toLocaleString(),
      icon: "mdi:star",
      color: "text-yellow-500",
    },
    {
      label: "Forks",
      value: forks?.toLocaleString() ?? GITHUB_LAST_SEEN_FORKS.toLocaleString(),
      icon: "mdi:source-fork",
      color: "text-blue-500",
    },
    {
      label: "Contributors",
      value: "17",
      icon: "mdi:account-group",
      color: "text-green-500",
    },
    {
      label: "Downloads",
      value: "40k+",
      imageUrl: "/api/images/hyprnote/icon.png",
      color: "text-purple-500",
    },
    {
      label: "Discord Members",
      value: "1k+",
      icon: "logos:discord-icon",
      color: "text-indigo-500",
    },
  ];

  return (
    <section>
      <div>
        <div className="px-6 py-12 lg:py-16">
          <h2 className="text-color mb-4 text-left font-mono text-3xl">
            How We're Doing
          </h2>
          <p className="text-fg-muted max-w-2xl text-left">
            Our progress is measured by the community we're building together.
          </p>
        </div>

        <div className="border-color-brand surface grid grid-cols-5 rounded-lg border">
          {stats.map((stat, index) => (
            <StatCard
              key={stat.label}
              label={stat.label}
              value={stat.value}
              icon={"icon" in stat ? stat.icon : undefined}
              imageUrl={"imageUrl" in stat ? stat.imageUrl : undefined}
              color={stat.color}
              hasBorder={index < 4}
            />
          ))}
        </div>
      </div>
    </section>
  );
}

const contributions = [
  {
    title: "Star Repository",
    description: "Show your support and help others discover Char",
    icon: "mdi:star",
    link: "https://github.com/fastrepl/char",
    linkText: "Star on GitHub",
  },
  {
    title: "Contribute Code",
    description: "Fix bugs, add features, or improve documentation",
    icon: "mdi:code-braces",
    link: "https://github.com/fastrepl/char/contribute",
    linkText: "View Issues",
  },
  {
    title: "Report Issues",
    description: "Help us improve by reporting bugs and suggesting features",
    icon: "mdi:bug",
    link: "https://github.com/fastrepl/char/issues",
    linkText: "Open Issue",
  },
  {
    title: "Help Translate",
    description: "Make Char accessible in your language",
    icon: "mdi:translate",
    link: "https://github.com/fastrepl/char",
    linkText: "Contribute Translations",
  },
  {
    title: "Spread the Word",
    description: "Share Char with your network and community",
    icon: "mdi:share-variant",
    link: "https://twitter.com/intent/tweet?text=Check%20out%Char%20-%20open%20source%20AI%20meeting%20notes%20that%20run%20locally!%20https://char.com",
    linkText: "Share on X",
  },
  {
    title: "Join Community",
    description: "Connect with other users and contributors",
    icon: "mdi:forum",
    link: "/discord",
    linkText: "Join Discord",
  },
];

function JoinMovementSection() {
  return (
    <section>
      <div>
        <div className="px-6 py-12 lg:py-16">
          <h2 className="text-color mb-4 text-left font-mono text-3xl">
            Be Part of the Movement
          </h2>
          <p className="text-fg-muted max-w-2xl text-left">
            Every contribution, no matter how small, helps build a more private
            future for AI.
          </p>
        </div>

        <div className="border-color-brand surface grid rounded-lg border sm:grid-cols-2 lg:grid-cols-3">
          {contributions.map((item, index) => {
            const isLastMobile = index === contributions.length - 1;
            const isLastRowSm =
              Math.floor(index / 2) === Math.ceil(contributions.length / 2) - 1;
            const isLastRowLg =
              Math.floor(index / 3) === Math.ceil(contributions.length / 3) - 1;

            return (
              <div
                key={item.title}
                className={cn([
                  "border-color-brand flex flex-col justify-between p-6",
                  !isLastMobile && "border-b",
                  !isLastRowSm && "sm:border-b",
                  isLastRowSm && "sm:border-b-0",
                  !isLastRowLg && "lg:border-b",
                  isLastRowLg && "lg:border-b-0",
                  index % 2 === 0 && "sm:border-r",
                  index % 3 !== 2 && "lg:border-r",
                  index % 3 === 2 && "lg:border-r-0",
                ])}
              >
                <div>
                  <h3 className="text-color mb-2 font-medium">{item.title}</h3>
                  <p className="text-fg-muted text-sm">{item.description}</p>
                </div>
                <div className="mt-4">
                  <a
                    href={item.link}
                    target="_blank"
                    rel="noopener noreferrer"
                    className={cn([
                      "inline-flex items-center justify-center gap-2 rounded-full px-4 py-2 text-sm font-medium",
                      "border-color-brand text-fg border",
                      "transition-transform hover:scale-[102%] active:scale-[98%]",
                    ])}
                  >
                    <Icon icon={item.icon} className="text-base" />
                    {item.linkText}
                  </a>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}
