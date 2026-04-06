"use client";

import { ChevronDown } from "lucide-react";

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
