import type { ComponentType } from "react";

import { ArrowRight } from "@anlg/ui/components/icons";
import { cn } from "@anlg/utils";

import { Image as OptimizedImage } from "@/components/image";
import { getPublicPageLinkHref } from "@/lib/seo";

const BLOG_IMAGE_WIDTH = 796;
const BLOG_IMAGE_BREAKPOINTS = [320, 480, 640, 796, 1080, 1280, 1592];
const BLOG_IMAGE_SIZES =
  "(min-width: 860px) 796px, (min-width: 768px) calc(100vw - 64px), calc(100vw - 40px)";

function Image({
  src,
  alt,
  className,
  width = BLOG_IMAGE_WIDTH,
  ...rest
}: {
  src: string;
  alt?: string;
  className?: string;
  width?: number;
  [k: string]: any;
}) {
  return (
    <OptimizedImage
      {...rest}
      src={src}
      alt={alt ?? ""}
      className={cn(["my-6 w-full rounded-md", className])}
      layout="constrained"
      width={width}
      breakpoints={BLOG_IMAGE_BREAKPOINTS}
      sizes={BLOG_IMAGE_SIZES}
    />
  );
}

function CtaCard({
  href,
  title,
  description,
  cta,
}: {
  href?: string;
  title?: string;
  description?: string;
  cta?: string;
}) {
  if (!href) return null;
  return (
    <a
      href={getPublicPageLinkHref(href)}
      className="my-6 block rounded-md border border-neutral-200 p-6 no-underline transition-colors hover:border-stone-400 hover:bg-stone-50"
    >
      {title && <div className="mb-1 text-base text-stone-800">{title}</div>}
      {description && (
        <div className="mb-3 text-sm text-neutral-600">{description}</div>
      )}
      {cta && (
        <div className="flex items-center gap-1 text-sm text-stone-600">
          {cta}
          <ArrowRight size={14} aria-hidden="true" />
        </div>
      )}
    </a>
  );
}

function Callout({
  type = "note",
  children,
}: {
  type?: string;
  children?: React.ReactNode;
}) {
  const tone =
    type === "warning"
      ? "bg-amber-50 border-amber-200"
      : type === "tip"
        ? "bg-emerald-50 border-emerald-200"
        : "bg-stone-50 border-stone-200";
  return (
    <aside className={`my-6 rounded-md border p-4 ${tone}`}>{children}</aside>
  );
}

function Clip({ src }: { src: string }) {
  const ytMatch = src.match(/(?:youtube\.com\/watch\?v=|youtu\.be\/)([^&]+)/);
  if (ytMatch) {
    return (
      <div className="my-6 aspect-video w-full overflow-hidden rounded-md border border-neutral-200">
        <iframe
          src={`https://www.youtube.com/embed/${ytMatch[1]}`}
          className="h-full w-full"
          allowFullScreen
        />
      </div>
    );
  }
  return null;
}

function BenchmarkChart({
  title,
  unit,
  subtitle,
  note,
  baselineLabel = "1.4.35 Tauri",
  candidateLabel = "2.0 GPUI",
  rows,
}: {
  title: string;
  unit: string;
  subtitle?: string;
  note?: string;
  baselineLabel?: string;
  candidateLabel?: string;
  rows: Array<{
    label: string;
    baseline: number;
    candidate: number;
    baselineText?: string;
    candidateText?: string;
    delta: string;
    regression?: boolean;
  }>;
}) {
  const barStyle = (val: number, rowMax: number) => ({
    width: `${(val / Math.max(rowMax, 1)) * 100}%`,
    minWidth: val > 0 ? "2px" : "0px",
  });

  return (
    <figure className="not-prose my-7 rounded-sm border border-[var(--color-border-subtle)] bg-[var(--color-surface-subtle)] p-4 text-[var(--color-fg)] md:p-5">
      <figcaption className="flex flex-wrap items-baseline justify-between gap-2 border-b border-[var(--color-border-subtle)] pb-2.5">
        <div className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
          <span className="text-sm font-semibold tracking-tight text-[var(--color-fg)]">
            {title}
          </span>
          <span className="font-sans text-xs text-[var(--color-fg-secondary)]">
            {subtitle ? `${subtitle} (${unit})` : unit}
          </span>
        </div>
        <div className="flex flex-wrap items-center gap-3 font-sans text-xs text-[var(--color-fg-secondary)]">
          <span className="inline-flex items-center gap-1.5">
            <span
              className="inline-block h-2.5 w-2.5 shrink-0 bg-red-600"
              aria-hidden="true"
            />
            {baselineLabel}
          </span>
          <span className="inline-flex items-center gap-1.5">
            <span
              className="inline-block h-2.5 w-2.5 shrink-0 bg-emerald-700"
              aria-hidden="true"
            />
            {candidateLabel}
          </span>
        </div>
      </figcaption>

      <div className="mt-3.5 space-y-3.5">
        {rows.map((row) => {
          const rowMax = Math.max(row.baseline, row.candidate, 1);
          const baselineText = row.baselineText ?? `${row.baseline} ${unit}`;
          const candidateText = row.candidateText ?? `${row.candidate} ${unit}`;
          return (
            <div key={row.label} className="space-y-1.5">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <span className="text-sm font-medium text-[var(--color-fg)]">
                  {row.label}
                </span>
                <span
                  className={cn([
                    "inline-flex items-center border px-1.5 py-0.5 font-sans text-xs font-medium tabular-nums",
                    row.regression
                      ? "border-red-300 bg-red-50 text-red-700"
                      : "border-emerald-300 bg-emerald-50 text-emerald-800",
                  ])}
                >
                  {row.delta}
                </span>
              </div>

              <div className="space-y-1">
                <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2.5">
                  <div className="h-2.5 w-full bg-[var(--color-border-subtle)]">
                    <div
                      className="h-full bg-red-600"
                      style={barStyle(row.baseline, rowMax)}
                      aria-hidden="true"
                    />
                  </div>
                  <span className="min-w-[6.5rem] text-right font-mono text-xs font-medium text-[var(--color-fg-secondary)] tabular-nums">
                    <span className="sr-only">{baselineLabel}: </span>
                    {baselineText}
                  </span>
                </div>

                <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2.5">
                  <div className="h-2.5 w-full bg-[var(--color-border-subtle)]">
                    <div
                      className="h-full bg-emerald-700"
                      style={barStyle(row.candidate, rowMax)}
                      aria-hidden="true"
                    />
                  </div>
                  <span className="min-w-[6.5rem] text-right font-mono text-xs font-medium text-[var(--color-fg)] tabular-nums">
                    <span className="sr-only">{candidateLabel}: </span>
                    {candidateText}
                  </span>
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {note && (
        <p className="mt-3 border-t border-[var(--color-border-subtle)] pt-2.5 font-sans text-xs leading-relaxed text-[var(--color-fg-secondary)]">
          {note}
        </p>
      )}
    </figure>
  );
}

const Noop = () => null;

function ArticleLink({ href, ...props }: React.ComponentProps<"a">) {
  return <a {...props} href={href ? getPublicPageLinkHref(href) : href} />;
}

function InlineCode({ children, ...props }: React.ComponentProps<"code">) {
  return (
    <code
      {...props}
      className={`rounded bg-stone-100 px-1.5 py-0.5 font-mono text-sm text-stone-800 ${
        props.className ?? ""
      }`}
    >
      {children}
    </code>
  );
}

export const mdxComponents: Record<string, ComponentType<any>> = {
  a: ArticleLink,
  Image,
  img: Image,
  CtaCard,
  Callout,
  Clip,
  BenchmarkChart,
  Aside: Noop,
  Figure: Noop,
  CodeBlock: Noop,
  ComparisonTable: Noop,
  Grid: Noop,
  Tabs: Noop,
  Video: Noop,
  code: InlineCode,
};
