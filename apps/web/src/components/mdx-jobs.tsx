"use client";

import { Icon } from "@iconify-icon/react";
import { ChevronDown } from "lucide-react";

import { cn } from "@hypr/utils";

import { AnimatedText } from "./animated-text";

export function ToolIcon({
  icon,
  className,
}: {
  icon: string;
  className?: string;
}) {
  return (
    <Icon
      icon={icon}
      className={cn(["mb-1 inline-block align-middle", className])}
    />
  );
}

export function ToolImage({
  src,
  alt,
  className = "",
}: {
  src: string;
  alt: string;
  className?: string;
}) {
  return (
    <img
      src={src}
      alt={alt}
      className={`inline-block align-middle ${className}`}
    />
  );
}

const TOOL_CONFIG: Record<
  string,
  { icon?: string; image?: { src: string; className: string } }
> = {
  tauri: { icon: "logos:tauri" },
  rust: { icon: "logos:rust" },
  typescript: { icon: "logos:typescript-icon" },
  zed: { icon: "devicon:zed" },
  cursor: { icon: "simple-icons:cursor" },
  claude: { icon: "logos:claude-icon" },
  devin: {
    image: {
      src: "https://mintcdn.com/cognitionai/k89q9Lsp7DOurdC0/logo/devin.png?fit=max&auto=format&n=k89q9Lsp7DOurdC0&q=85&s=e83fbc727ea2cae8f1b80442fa772c50",
      className: "size-5 -mx-1 mb-0.75",
    },
  },
  graphite: { icon: "simple-icons:graphite" },
  gitbutler: {
    image: {
      src: "https://dl.flathub.org/media/com/gitbutler/gitbutler/86fc196ac5615bc7ed82d530d29309c9/icons/128x128@2/com.gitbutler.gitbutler.png",
      className: "size-5 -mx-0.5 mb-0.5",
    },
  },
  figma: { icon: "logos:figma" },
  slack: { icon: "logos:slack-icon" },
  github: { icon: "logos:github-icon" },
  hyprnote: {
    image: {
      src: "/api/images/hyprnote/icon.png",
      className: "size-5 -mx-0.5 mb-0.5 rounded",
    },
  },
};

const TOOL_NAMES: Record<string, string> = {
  tauri: "Tauri",
  rust: "Rust",
  typescript: "TypeScript",
  zed: "Zed",
  cursor: "Cursor",
  claude: "Claude Code",
  devin: "Devin",
  graphite: "Graphite",
  gitbutler: "GitButler",
  figma: "Figma",
  slack: "Slack",
  github: "GitHub",
  hyprnote: "Char",
};

function ToolWithIcon({ tool }: { tool: string }) {
  const config = TOOL_CONFIG[tool];
  const name = TOOL_NAMES[tool] || tool;

  if (!config) {
    return <span>{name}</span>;
  }

  if (config.icon) {
    return (
      <>
        <Icon icon={config.icon} className="mb-1 inline-block align-middle" />{" "}
        {name}
      </>
    );
  }

  if (config.image) {
    return (
      <>
        <img
          src={config.image.src}
          alt={name}
          className={`inline-block align-middle ${config.image.className}`}
        />{" "}
        {name}
      </>
    );
  }

  return <span>{name}</span>;
}

export function ToolStack({ tools }: { tools: string[] }) {
  return (
    <p className="text-neutral-600">
      Stack:{" "}
      {tools.map((tool, index) => (
        <span key={tool}>
          <ToolWithIcon tool={tool} />
          {index < tools.length - 1 && ", "}
        </span>
      ))}
      .
    </p>
  );
}

export function EditorStack({ tools }: { tools: string[] }) {
  return (
    <p className="text-neutral-600">
      Editors like{" "}
      {tools.map((tool, index) => (
        <span key={tool}>
          <ToolWithIcon tool={tool} />
          {index < tools.length - 1 && " and "}
        </span>
      ))}
      .
    </p>
  );
}

export function AIToolStack({ tools }: { tools: string[] }) {
  return (
    <p className="text-neutral-600">
      AI tools like{" "}
      {tools.map((tool, index) => (
        <span key={tool}>
          <ToolWithIcon tool={tool} />
          {index < tools.length - 1 && " and "}
        </span>
      ))}
      .
    </p>
  );
}

export function GitToolStack({ tools }: { tools: string[] }) {
  return (
    <p className="text-neutral-600">
      Git workflows via{" "}
      {tools.map((tool, index) => (
        <span key={tool}>
          <ToolWithIcon tool={tool} />
          {index < tools.length - 1 && " and "}
        </span>
      ))}
      .
    </p>
  );
}

export function AnimatedJobText({
  text,
  className = "",
}: {
  text: string;
  className?: string;
}) {
  return <AnimatedText text={text} className={className} />;
}

export function GitHubMention({
  username,
  name,
}: {
  username: string;
  name?: string;
}) {
  const avatarUrl = `https://github.com/${username}.png?size=48`;
  const profileUrl = `https://github.com/${username}`;

  return (
    <a
      href={profileUrl}
      target="_blank"
      rel="noopener noreferrer"
      className={[
        "inline-flex items-center gap-1 align-middle whitespace-nowrap",
        "font-semibold text-inherit",
        "underline underline-offset-2 decoration-neutral-400",
        "hover:decoration-neutral-600 transition-colors",
      ].join(" ")}
    >
      <img
        src={avatarUrl}
        alt={name ?? username}
        className="size-5 shrink-0 rounded-full no-underline"
      />
      {name ?? `@${username}`}
    </a>
  );
}

export function HyprnoteIcon() {
  return (
    <img
      src="/api/images/hyprnote/icon.png"
      alt="Char"
      className="mb-0.75 inline-block size-4.5 rounded-md align-middle"
    />
  );
}

export function CharIcon() {
  return <HyprnoteIcon />;
}

export function ChevronIcon() {
  return (
    <ChevronDown className="size-4 text-neutral-400 transition-transform group-open:rotate-180" />
  );
}

export function Collapsible({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <details className="group overflow-hidden rounded-lg border border-neutral-200 bg-neutral-50">
      <summary className="flex cursor-pointer items-center justify-between px-4 py-3 font-medium text-neutral-700 transition-colors hover:bg-neutral-100">
        <span>{title}</span>
        <ChevronDown className="size-4 text-neutral-400 transition-transform group-open:rotate-180" />
      </summary>
      <div className="border-t border-neutral-200 px-4 py-3 text-neutral-600 [&_li]:pl-1 [&_p]:mb-4 [&_p+p]:mt-4 [&_ul]:list-disc [&_ul]:space-y-2 [&_ul]:pl-5 [&_ul+p]:mt-6 [&>*:first-child]:mt-0 [&>*:last-child]:mb-0">
        {children}
      </div>
    </details>
  );
}

export function FAQItem({
  question,
  children,
}: {
  question: string;
  children: React.ReactNode;
}) {
  return (
    <details className="group last:border-b-0">
      <summary className="text-color flex cursor-pointer list-none items-start justify-between gap-4 py-6 pr-4 text-lg transition-colors hover:text-neutral-600 [&::-webkit-details-marker]:hidden">
        <span>{question}</span>
        <ChevronDown className="text-color-muted mt-1 size-4 shrink-0 transition-transform group-open:rotate-180" />
      </summary>
      <div className="text-color [&_ul:list-disc pb-4 [&_li]:pl-1 [&_p]:mb-3 [&_p:last-child]:mb-0 [&_ul]:space-y-2 [&_ul]:pl-5 [&>*:first-child]:mt-0 [&>*:last-child]:mb-0">
        {children}
      </div>
    </details>
  );
}

export function FAQ({ children }: { children: React.ReactNode }) {
  return (
    <div className="border-color-brand bg-surface w-full divide-y divide-neutral-200 rounded-lg border px-4">
      {children}
    </div>
  );
}

export function CompanyStats() {
  return (
    <>
      7.4k GitHub stars, 9.2% conversion rate, 50k app downloads, 30% WoW DAU
      growth, 38% week-5 retention, 10% MoM MRR growth - all organically.{" "}
      <AnimatedJobText
        text="We're not stopping here - this is just the start."
        className="text-neutral-400"
      />
      <br />
      <br />
      We're also backed by{" "}
      <a
        href="https://www.ycombinator.com/companies/hyprnote"
        target="_blank"
        rel="noopener noreferrer"
        className="underline decoration-neutral-400 underline-offset-2 transition-colors hover:decoration-neutral-600"
      >
        Y Combinator
      </a>
      ,{" "}
      <a
        href="https://www.pioneerfund.vc"
        target="_blank"
        rel="noopener noreferrer"
        className="underline decoration-neutral-400 underline-offset-2 transition-colors hover:decoration-neutral-600"
      >
        Pioneer
      </a>
      , and{" "}
      <a
        href="https://www.trac.vc"
        target="_blank"
        rel="noopener noreferrer"
        className="underline decoration-neutral-400 underline-offset-2 transition-colors hover:decoration-neutral-600"
      >
        TRAC
      </a>
      , with participations from awesome angels like{" "}
      <a
        href="https://www.linkedin.com/in/tobiaslutke/"
        target="_blank"
        rel="noopener noreferrer"
        className="underline decoration-neutral-400 underline-offset-2 transition-colors hover:decoration-neutral-600"
      >
        Tobi Lütke (Shopify)
      </a>
      ,{" "}
      <a
        href="https://www.linkedin.com/in/zachlloyd/"
        target="_blank"
        rel="noopener noreferrer"
        className="underline decoration-neutral-400 underline-offset-2 transition-colors hover:decoration-neutral-600"
      >
        Zach Lloyd (Warp)
      </a>
      ,{" "}
      <a
        href="https://www.linkedin.com/in/henri-stern/"
        target="_blank"
        rel="noopener noreferrer"
        className="underline decoration-neutral-400 underline-offset-2 transition-colors hover:decoration-neutral-600"
      >
        Henri Stern (Privy)
      </a>
      ,{" "}
      <a
        href="https://www.linkedin.com/in/thom-wolf/"
        target="_blank"
        rel="noopener noreferrer"
        className="underline decoration-neutral-400 underline-offset-2 transition-colors hover:decoration-neutral-600"
      >
        Thomas Wolf (Hugging Face)
      </a>
      ,{" "}
      <a
        href="https://www.linkedin.com/in/j-hawkins/"
        target="_blank"
        rel="noopener noreferrer"
        className="underline decoration-neutral-400 underline-offset-2 transition-colors hover:decoration-neutral-600"
      >
        James Hawkins (PostHog)
      </a>
      ,{" "}
      <a
        href="https://www.linkedin.com/in/dknecht/"
        target="_blank"
        rel="noopener noreferrer"
        className="underline decoration-neutral-400 underline-offset-2 transition-colors hover:decoration-neutral-600"
      >
        Dane Knecht (Cloudflare)
      </a>
      ... the list goes on.
    </>
  );
}

export const jobsMdxComponents = {
  ToolIcon,
  ToolImage,
  ToolStack,
  EditorStack,
  AIToolStack,
  GitToolStack,
  AnimatedJobText,
  GitHubMention,
  CharIcon,
  HyprnoteIcon,
  ChevronIcon,
  Collapsible,
  FAQ,
  FAQItem,
  CompanyStats,
};
