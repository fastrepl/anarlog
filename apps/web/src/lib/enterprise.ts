export const BOOK_CALL_URL = "https://cal.com/team/fastrepl/hi";
export const SECURITY_REPORT_EMAIL = "founders@fastrepl.com";
export const SECURITY_ADVISORY_URL =
  "https://github.com/fastrepl/anarlog/security/advisories/new";
export const PROCUREMENT_EMAIL = "founders@anarlog.so";

export const ENTERPRISE_EVENTS = {
  pageViewed: "enterprise_page_viewed",
  securityPageViewed: "security_page_viewed",
  ctaClicked: "enterprise_cta_clicked",
} as const;

export type EnterpriseCta =
  | "book_call"
  | "security"
  | "enterprise"
  | "privacy"
  | "terms"
  | "pricing"
  | "docs"
  | "dpa"
  | "security_report";

export type EnterpriseCtaLocation =
  | "hero"
  | "security_review"
  | "pilot"
  | "talk"
  | "footer"
  | "packet";

export type EnterpriseSurface = "enterprise" | "security";
