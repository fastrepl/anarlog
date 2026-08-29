import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { cn } from "@anlg/utils";

import { useAnalytics } from "@/hooks/use-posthog";
import {
  ENTERPRISE_EVENTS,
  type EnterpriseCta,
  type EnterpriseCtaLocation,
  type EnterpriseSurface,
} from "@/lib/enterprise";

export function EnterpriseCtaLink({
  to,
  href,
  cta,
  location,
  page,
  children,
  className,
}: {
  to?: "/security/" | "/privacy/" | "/terms/" | "/pricing/" | "/enterprise/";
  href?: string;
  cta: EnterpriseCta;
  location: EnterpriseCtaLocation;
  page: EnterpriseSurface;
  children: ReactNode;
  className?: string;
}) {
  const { track } = useAnalytics();
  const classes = cn([
    "text-sm text-[#756b5d] underline decoration-[#d9cdb8] underline-offset-4 transition-colors hover:text-[#181613]",
    className,
  ]);
  const onClick = () =>
    track(ENTERPRISE_EVENTS.ctaClicked, { cta, location, page });

  if (to) {
    return (
      <Link to={to} onClick={onClick} className={classes}>
        {children}
      </Link>
    );
  }

  return (
    <a
      href={href}
      onClick={onClick}
      className={classes}
      {...(href?.startsWith("http")
        ? { target: "_blank", rel: "noopener noreferrer" }
        : {})}
    >
      {children}
    </a>
  );
}
