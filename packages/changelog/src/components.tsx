import { cn } from "@hypr/utils";

export const changelogComponents = {
  h2: ({ children }: { children?: React.ReactNode }) => (
    <h2 className="mt-6 mb-3 pt-6 text-base font-semibold text-amber-950 first:mt-0 first:pt-0">
      {children}
    </h2>
  ),
  h3: ({ children }: { children?: React.ReactNode }) => (
    <h3 className="mt-5 mb-2 text-sm font-semibold text-amber-950">
      {children}
    </h3>
  ),
  h4: ({ children }: { children?: React.ReactNode }) => (
    <h4 className="mt-4 mb-2 text-sm font-medium text-amber-950">{children}</h4>
  ),
  p: ({ children }: { children?: React.ReactNode }) => (
    <p className="my-2 text-stone-600">{children}</p>
  ),
  strong: ({ children }: { children?: React.ReactNode }) => (
    <strong className="font-semibold text-stone-800">{children}</strong>
  ),
  em: ({ children }: { children?: React.ReactNode }) => (
    <em className="italic">{children}</em>
  ),
  code: ({ children }: { children?: React.ReactNode }) => (
    <code className="rounded bg-amber-50 px-1 py-0.5 text-[0.85em] text-amber-800">
      {children}
    </code>
  ),
  ul: ({ children }: { children?: React.ReactNode }) => (
    <ul className="my-2 list-disc pl-6 text-stone-600">{children}</ul>
  ),
  ol: ({ children }: { children?: React.ReactNode }) => (
    <ol className="my-2 list-decimal pl-6 text-stone-600">{children}</ol>
  ),
  li: ({ children }: { children?: React.ReactNode }) => (
    <li className="my-0.5">{children}</li>
  ),
  blockquote: ({ children }: { children?: React.ReactNode }) => (
    <blockquote className="my-4 border-l-2 border-amber-200 pl-4 text-stone-500 italic">
      {children}
    </blockquote>
  ),
  a: ({ href, children }: { href?: string; children?: React.ReactNode }) => (
    <a
      className="text-amber-700 underline decoration-amber-300 underline-offset-2 hover:text-amber-900 hover:decoration-amber-500"
      href={href}
      target="_blank"
      rel="noopener noreferrer"
    >
      {children}
    </a>
  ),
  img: ({ src, alt }: { src?: string; alt?: string }) => (
    <img
      src={src}
      alt={alt}
      className="my-6 rounded-lg border border-stone-200"
    />
  ),
  banner: ({
    title,
    variant,
    children,
  }: {
    title?: string;
    variant?: string;
    children?: React.ReactNode;
  }) => (
    <div
      className={cn([
        "mb-2 rounded-xl border px-5 pt-4 pb-4",
        variant === "warning"
          ? "border-amber-300 bg-amber-50 text-amber-900"
          : "border-amber-200 bg-amber-50/60 text-stone-800",
      ])}
    >
      {title && (
        <div className="mb-1 text-sm font-semibold text-amber-900">{title}</div>
      )}
      <div className="text-sm text-stone-600">{children}</div>
    </div>
  ),
};
