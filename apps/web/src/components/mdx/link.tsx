import { Link } from "@tanstack/react-router";

import { cn, withCharUtm } from "@hypr/utils";

const linkClassName =
  "underline underline-offset-2 decoration-neutral-400 hover:decoration-neutral-600 transition-colors";

function normalizePathname(pathname: string) {
  return pathname === "/" ? pathname : pathname.replace(/\/+$/, "") || "/";
}

function getTrackedWebsiteHref(href: string, utmMedium?: "blog" | "docs") {
  if (!utmMedium) {
    return href;
  }

  let url: URL;
  try {
    url = new URL(href);
  } catch {
    return href;
  }

  if (!["char.com", "www.char.com"].includes(url.hostname)) {
    return href;
  }

  const pathname = normalizePathname(url.pathname);
  const shouldTrack =
    utmMedium === "blog"
      ? pathname === "/" ||
        pathname === "/download" ||
        pathname.startsWith("/download/")
      : pathname === "/download" ||
        pathname.startsWith("/download/") ||
        pathname === "/founders";

  if (!shouldTrack) {
    return href;
  }

  return withCharUtm(href, { source: "website", medium: utmMedium });
}

export function createMDXLink({
  utmMedium,
}: {
  utmMedium?: "blog" | "docs";
} = {}) {
  return function MDXLink({
    href,
    children,
    className,
    ...props
  }: React.AnchorHTMLAttributes<HTMLAnchorElement>) {
    if (!href) {
      return <span {...props}>{children}</span>;
    }

    const resolvedHref = getTrackedWebsiteHref(href, utmMedium);
    const isHyprnoteUrl = resolvedHref.startsWith("https://hyprnote.com");
    const isInternalPath =
      resolvedHref.startsWith("/") || resolvedHref.startsWith(".");
    const isAnchor = resolvedHref.startsWith("#");

    if (isHyprnoteUrl) {
      const relativePath =
        resolvedHref.replace("https://hyprnote.com", "") || "/";
      return (
        <Link
          to={relativePath}
          className={cn([linkClassName, className])}
          {...props}
        >
          {children}
        </Link>
      );
    }

    if (isAnchor) {
      return (
        <a
          href={resolvedHref}
          className={cn([linkClassName, className])}
          {...props}
        >
          {children}
        </a>
      );
    }

    if (isInternalPath) {
      return (
        <Link
          to={resolvedHref}
          className={cn([linkClassName, className])}
          {...props}
        >
          {children}
        </Link>
      );
    }

    return (
      <a
        href={resolvedHref}
        target="_blank"
        rel="noopener noreferrer"
        className={cn([linkClassName, className])}
        {...props}
      >
        {children}
      </a>
    );
  };
}

export const MDXLink = createMDXLink();
