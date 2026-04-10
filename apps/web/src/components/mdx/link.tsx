import { Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

const linkClassName =
  "underline underline-offset-2 decoration-neutral-400 hover:decoration-neutral-600 transition-colors";

export function MDXLink({
  href,
  children,
  className,
  ...props
}: React.AnchorHTMLAttributes<HTMLAnchorElement>) {
  if (!href) {
    return <span {...props}>{children}</span>;
  }

  const isCharUrl = href.startsWith("https://char.com");
  const isInternalPath = href.startsWith("/") || href.startsWith(".");
  const isAnchor = href.startsWith("#");

  if (isCharUrl) {
    const relativePath = href.replace("https://char.com", "") || "/";
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
      <a href={href} className={cn([linkClassName, className])} {...props}>
        {children}
      </a>
    );
  }

  if (isInternalPath) {
    return (
      <Link to={href} className={cn([linkClassName, className])} {...props}>
        {children}
      </Link>
    );
  }

  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className={cn([linkClassName, className])}
      {...props}
    >
      {children}
    </a>
  );
}
