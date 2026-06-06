import { isValidElement } from "react";
import { Streamdown } from "streamdown";

import { cn } from "@hypr/utils";

function flattenTextContent(node: React.ReactNode): string {
  if (node == null || typeof node === "boolean") {
    return "";
  }

  if (
    typeof node === "string" ||
    typeof node === "number" ||
    typeof node === "bigint"
  ) {
    return String(node);
  }

  if (Array.isArray(node)) {
    return node.map(flattenTextContent).join("");
  }

  if (isValidElement<{ children?: React.ReactNode }>(node)) {
    return flattenTextContent(node.props.children);
  }

  return "";
}

const changelogLinkClassName =
  "text-blue-600 underline decoration-blue-400/40 underline-offset-2 hover:text-blue-700 dark:text-blue-400 dark:decoration-blue-500/50 dark:hover:text-blue-300";

const changelogBodyClassName = "text-foreground/85";

const baseChangelogComponents = {
  h2: ({ children }: { children?: React.ReactNode }) => (
    <h2 className="text-foreground mt-6 mb-3 pt-6 text-base font-semibold first:mt-0 first:pt-0">
      {children}
    </h2>
  ),
  h3: ({ children }: { children?: React.ReactNode }) => (
    <h3 className="text-foreground mt-5 mb-2 text-sm font-semibold">
      {children}
    </h3>
  ),
  h4: ({ children }: { children?: React.ReactNode }) => (
    <h4 className="text-foreground mt-4 mb-2 text-sm font-medium">
      {children}
    </h4>
  ),
  p: ({ children }: { children?: React.ReactNode }) => (
    <p className={cn(["my-2", changelogBodyClassName])}>{children}</p>
  ),
  strong: ({ children }: { children?: React.ReactNode }) => (
    <strong className="text-foreground font-semibold">{children}</strong>
  ),
  em: ({ children }: { children?: React.ReactNode }) => (
    <em className="italic">{children}</em>
  ),
  code: ({ children }: { children?: React.ReactNode }) => (
    <code className="bg-muted text-foreground rounded px-1 py-0.5 text-[0.85em]">
      {children}
    </code>
  ),
  ul: ({ children }: { children?: React.ReactNode }) => (
    <ul className={cn(["my-2 list-disc pl-6", changelogBodyClassName])}>
      {children}
    </ul>
  ),
  ol: ({ children }: { children?: React.ReactNode }) => (
    <ol className={cn(["my-2 list-decimal pl-6", changelogBodyClassName])}>
      {children}
    </ol>
  ),
  li: ({ children }: { children?: React.ReactNode }) => (
    <li className="my-0.5">{children}</li>
  ),
  blockquote: ({ children }: { children?: React.ReactNode }) => (
    <blockquote className="border-border text-muted-foreground my-4 border-l-2 pl-4 italic">
      {children}
    </blockquote>
  ),
  a: ({ href, children }: { href?: string; children?: React.ReactNode }) => (
    <a
      className={changelogLinkClassName}
      href={href}
      target="_blank"
      rel="noopener noreferrer"
    >
      {children}
    </a>
  ),
  img: ({ src, alt }: { src?: string; alt?: string }) => (
    <img src={src} alt={alt} className="border-border my-6 rounded-lg border" />
  ),
};

export const changelogComponents = {
  ...baseChangelogComponents,
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
          ? "border-amber-300 bg-amber-50 text-amber-900 dark:border-amber-800 dark:bg-amber-950/40 dark:text-amber-100"
          : variant === "info"
            ? "border-blue-300 bg-blue-50 text-blue-900 dark:border-blue-800 dark:bg-blue-950/40 dark:text-blue-100"
            : "border-border bg-muted text-foreground",
      ])}
    >
      {title && (
        <div
          className={cn([
            "mb-1 text-sm font-semibold",
            variant === "warning"
              ? "text-amber-900 dark:text-amber-100"
              : variant === "info"
                ? "text-blue-900 dark:text-blue-100"
                : "text-foreground",
          ])}
        >
          {title}
        </div>
      )}
      <div
        className={cn([
          "text-sm [&_p:last-child]:mb-0 [&_ul:last-child]:mb-0",
          changelogBodyClassName,
        ])}
      >
        <Streamdown
          components={baseChangelogComponents}
          isAnimating={false}
          linkSafety={{ enabled: false }}
        >
          {flattenTextContent(children)}
        </Streamdown>
      </div>
    </div>
  ),
};
