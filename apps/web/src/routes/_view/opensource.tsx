import { Icon } from "@iconify-icon/react";
import { createFileRoute } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";
import { Fragment, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import { DownloadButton } from "@/components/download-button";
import { Image } from "@/components/image";
import { SlashSeparator } from "@/components/slash-separator";
import {
  GITHUB_LAST_SEEN_FORKS,
  GITHUB_LAST_SEEN_STARS,
  Stargazer,
  useGitHubStargazers,
  useGitHubStats,
} from "@/queries";
import { CTASection } from "@/routes/_view/index";

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
    <div
      className="min-h-screen bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <HeroSection />
        <SlashSeparator />
        <LetterSection />
        <SlashSeparator />
        <TechStackSection />
        <SlashSeparator />
        <SponsorsSection />
        <SlashSeparator />
        <ProgressSection />
        <SlashSeparator />
        <JoinMovementSection />
        <SlashSeparator />
        <CTASection heroInputRef={heroInputRef} />
      </div>
    </div>
  );
}

function StargazerAvatar({ stargazer }: { stargazer: Stargazer }) {
  return (
    <a
      href={`https://github.com/${stargazer.username}`}
      target="_blank"
      rel="noopener noreferrer"
      className="block size-14 shrink-0 overflow-hidden rounded-xs border border-neutral-100/50 bg-neutral-100 transition-all hover:scale-110 hover:border-neutral-400 hover:opacity-100"
    >
      <img
        src={stargazer.avatar}
        alt={`${stargazer.username}'s avatar`}
        className="h-full w-full object-cover"
        loading="lazy"
      />
    </a>
  );
}

function StargazersGrid({ stargazers }: { stargazers: Stargazer[] }) {
  const rows = 10;
  const cols = 20;

  return (
    <div className="pointer-events-none absolute inset-0 overflow-hidden">
      <div className="absolute inset-0 flex flex-col justify-center gap-1 px-4 opacity-40">
        {Array.from({ length: rows }).map((_, rowIndex) => (
          <div key={rowIndex} className="flex justify-center gap-1">
            {Array.from({ length: cols }).map((_, colIndex) => {
              const index = (rowIndex * cols + colIndex) % stargazers.length;
              const stargazer = stargazers[index];
              const delay = Math.random() * 3;

              return (
                <div
                  key={`${rowIndex}-${colIndex}`}
                  className="animate-fade-in-out pointer-events-auto"
                  style={{
                    animationDelay: `${delay}s`,
                    animationDuration: "3s",
                  }}
                >
                  <StargazerAvatar stargazer={stargazer} />
                </div>
              );
            })}
          </div>
        ))}
      </div>
    </div>
  );
}

function HeroSection() {
  const { data: stargazers = [] } = useGitHubStargazers();

  return (
    <div className="relative overflow-hidden bg-linear-to-b from-stone-50/30 to-stone-100/30">
      {stargazers.length > 0 && <StargazersGrid stargazers={stargazers} />}
      <div className="relative z-10 px-6 py-12 lg:py-20">
        <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_800px_400px_at_50%_50%,white_0%,rgba(255,255,255,0.8)_40%,transparent_70%)]" />
        <header className="relative mx-auto max-w-4xl py-6 text-center">
          <h1 className="mb-6 font-serif text-4xl text-stone-700 sm:text-5xl lg:text-6xl">
            Built in the open,
            <br />
            for everyone
          </h1>
          <p className="mx-auto max-w-3xl text-lg leading-relaxed text-neutral-600 sm:text-xl">
            Char is fully open source under GPL-3.0. Every line of code is
            auditable, every decision is transparent, and every user has the
            freedom to inspect, modify, and contribute.
          </p>
          <div className="mt-8 flex flex-col justify-center gap-4 sm:flex-row">
            <a
              href="https://github.com/fastrepl/char"
              target="_blank"
              rel="noopener noreferrer"
              className={cn([
                "inline-flex items-center justify-center gap-2 rounded-full px-8 py-3 font-medium",
                "bg-linear-to-t from-neutral-800 to-neutral-700 text-white",
                "transition-transform hover:scale-105 active:scale-95",
              ])}
            >
              <Icon icon="mdi:github" className="text-lg" />
              View on GitHub
            </a>
            <DownloadButton />
          </div>
        </header>
      </div>
    </div>
  );
}

function LetterSection() {
  return (
    <section className="bg-[linear-gradient(to_right,#fafafa_1px,transparent_1px),linear-gradient(to_bottom,#fafafa_1px,transparent_1px)] bg-size-[24px_24px] bg-position-[12px_12px,12px_12px] px-6 py-16 lg:py-24">
      <div className="mx-auto max-w-3xl">
        <div className="mb-8 text-center">
          <span className="font-mono text-sm font-medium tracking-widest text-neutral-500 uppercase">
            A letter from our team
          </span>
        </div>

        <article>
          <h1 className="mb-12 text-center font-serif text-3xl text-stone-700 sm:text-4xl lg:text-5xl">
            Why Open Source is Inevitable
            <br />
            in the Age of AI
          </h1>

          <div className="flex flex-col gap-6 leading-relaxed text-neutral-700">
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
                  className="rounded-full border border-neutral-100 object-cover"
                />
                <Image
                  src="/api/images/team/yujong.png"
                  alt="Yujong Lee"
                  width={32}
                  height={32}
                  className="rounded-full border border-neutral-100 object-cover"
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
        <div className="py-12 lg:py-16">
          <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
            Our Tech Stack
          </h2>
          <p className="mx-auto max-w-2xl text-center text-neutral-600">
            Built with modern, privacy-respecting technologies that run locally
            on your device.
          </p>
        </div>

        <div className="grid grid-cols-6">
          {techStack.map((section) => {
            return (
              <Fragment key={section.category}>
                <div className="col-span-6 border-t border-b border-neutral-100 bg-stone-50/50 p-6">
                  <h3 className="font-serif text-xl text-stone-700">
                    {section.category}
                  </h3>
                </div>
                {section.items.map((tech, techIndex) => {
                  const itemCount = section.items.length;
                  const posInRow2 = techIndex % 2;
                  const posInRow3 = techIndex % 3;
                  const rowIn2Col = Math.floor(techIndex / 2);
                  const rowIn3Col = Math.floor(techIndex / 3);
                  const totalRows2Col = Math.ceil(itemCount / 2);
                  const totalRows3Col = Math.ceil(itemCount / 3);
                  const isLastItemMobile = techIndex === itemCount - 1;
                  const isLastRowSm = rowIn2Col === totalRows2Col - 1;
                  const isLastRowLg = rowIn3Col === totalRows3Col - 1;

                  const hasBorderBMobile = !isLastItemMobile;
                  const hasBorderRSm = posInRow2 < 1;
                  const hasBorderRLg = posInRow3 < 2;
                  const hasBorderBSm = !isLastRowSm;
                  const hasBorderBLg = !isLastRowLg;

                  return (
                    <a
                      key={tech.name}
                      href={tech.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className={cn([
                        "col-span-6 sm:col-span-3 lg:col-span-2",
                        "border-neutral-100 p-6",
                        "group transition-all hover:bg-stone-50/30",
                        hasBorderBMobile && "border-b",
                        hasBorderRSm && "sm:border-r",
                        !hasBorderBSm && "sm:border-b-0",
                        hasBorderBSm && "sm:border-b",
                        !hasBorderRLg && "lg:border-r-0",
                        hasBorderRLg && "lg:border-r",
                        !hasBorderBLg && "lg:border-b-0",
                        hasBorderBLg && "lg:border-b",
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
                            className="text-2xl text-stone-700 transition-colors group-hover:text-stone-800"
                          />
                        )}
                        <h4 className="font-medium text-stone-700 transition-colors group-hover:text-stone-800">
                          {tech.name}
                        </h4>
                      </div>
                      <p className="text-sm text-neutral-600">
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
        <div className="py-12 lg:py-16">
          <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
            Paying It Forward
          </h2>
          <p className="mx-auto max-w-2xl text-center text-neutral-600">
            We love giving back to the community that makes Char possible. As we
            grow, we hope to sponsor even more projects and creators.
          </p>
        </div>

        <div className="grid grid-cols-6">
          <div className="col-span-6 border-t border-b border-neutral-100 bg-stone-50/50 p-6">
            <h3 className="font-serif text-xl text-stone-700">
              Projects We Sponsor
            </h3>
          </div>
          {sponsors.map((sponsor, index) => {
            const hasBorderR = index < sponsors.length - 1;

            return (
              <a
                key={sponsor.name}
                href={sponsor.url}
                target="_blank"
                rel="noopener noreferrer"
                className={cn([
                  "col-span-6 sm:col-span-3 lg:col-span-2",
                  "border-neutral-100 p-6",
                  "group transition-all hover:bg-stone-50/30",
                  index % 2 === 0 && "sm:border-r",
                  index > 0 && "border-t sm:border-t-0",
                  hasBorderR && "lg:border-r",
                ])}
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
                      className="text-2xl text-stone-700 transition-colors group-hover:text-stone-800"
                    />
                  )}
                  <h4 className="font-medium text-stone-700 transition-colors group-hover:text-stone-800">
                    {sponsor.name}
                  </h4>
                </div>
                <p className="text-sm text-neutral-600">
                  {sponsor.description}
                </p>
              </a>
            );
          })}
          <div className="col-span-6 flex flex-col gap-4 border-t border-neutral-100 bg-stone-50/50 p-6 lg:flex-row lg:items-center lg:justify-between">
            <div>
              <h3 className="font-serif text-xl text-stone-700">
                We Appreciate Your Support
              </h3>
              <p className="mt-2 text-sm text-neutral-600">
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
                "bg-linear-to-t from-pink-100 to-white text-stone-700",
                "border border-pink-200",
                "transition-transform hover:scale-105 active:scale-95",
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
        "relative flex h-32 flex-col justify-between gap-3 border-neutral-100 p-6 text-center",
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
        <p className="text-2xl font-bold text-stone-700">{value}</p>
        <p className="text-sm text-neutral-500">{label}</p>
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
        <div className="py-12 lg:py-16">
          <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
            How We're Doing
          </h2>
          <p className="mx-auto max-w-2xl text-center text-neutral-600">
            Our progress is measured by the community we're building together.
          </p>
        </div>

        <div className="grid grid-cols-5 border-t border-neutral-100">
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
    <section className="bg-stone-50/30">
      <div>
        <div className="px-6 py-12 lg:py-16">
          <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
            Be Part of the Movement
          </h2>
          <p className="mx-auto max-w-2xl text-center text-neutral-600">
            Every contribution, no matter how small, helps build a more private
            future for AI.
          </p>
        </div>

        <div className="grid border-t border-neutral-100 sm:grid-cols-2 lg:grid-cols-3">
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
                  "flex flex-col justify-between border-neutral-100 p-6",
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
                  <h3 className="mb-2 font-medium text-stone-700">
                    {item.title}
                  </h3>
                  <p className="text-sm text-neutral-600">{item.description}</p>
                </div>
                <div className="mt-4">
                  <a
                    href={item.link}
                    target="_blank"
                    rel="noopener noreferrer"
                    className={cn([
                      "inline-flex items-center justify-center gap-2 rounded-full px-4 py-2 text-sm font-medium",
                      "bg-linear-to-t from-neutral-100 to-white text-stone-700",
                      "border border-neutral-200",
                      "transition-transform hover:scale-105 active:scale-95",
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
