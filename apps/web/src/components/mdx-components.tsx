import type { ComponentType } from "react";

function Image({
  src,
  alt,
  ...rest
}: {
  src: string;
  alt?: string;
  [k: string]: any;
}) {
  return (
    <img
      src={src}
      alt={alt ?? ""}
      className="my-6 w-full rounded-md border border-neutral-200"
      {...rest}
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
      href={href}
      className="my-6 block rounded-md border border-neutral-200 p-6 no-underline transition-colors hover:border-stone-400 hover:bg-stone-50"
    >
      {title && (
        <div className="mb-1 font-mono text-base text-stone-800">{title}</div>
      )}
      {description && (
        <div className="mb-3 text-sm text-neutral-600">{description}</div>
      )}
      {cta && <div className="text-sm text-stone-600">{cta} →</div>}
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

const Noop = () => null;

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
  Image,
  img: Image,
  CtaCard,
  Callout,
  Clip,
  Aside: Noop,
  Figure: Noop,
  CodeBlock: Noop,
  ComparisonTable: Noop,
  Grid: Noop,
  Tabs: Noop,
  Video: Noop,
  code: InlineCode,
};
