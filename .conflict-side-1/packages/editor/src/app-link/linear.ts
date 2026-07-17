export type LinearLinkKind =
  | "document"
  | "issue"
  | "initiative"
  | "project"
  | "route"
  | "team"
  | "view"
  | "workspace";

export interface LinearAttrs {
  provider: "linear";
  kind: LinearLinkKind;
  url: string;
  workspace?: string;
  resourceTitle?: string;
  resourceId?: string;
}

const HOSTS = new Set(["linear.app", "www.linear.app"]);

function decodePathSegment(value: string | undefined): string {
  if (!value) {
    return "";
  }

  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function titleFromSlug(value: string | undefined): string {
  const decoded = decodePathSegment(value);
  if (!decoded) {
    return "";
  }

  const title = decoded
    .replace(/[a-f0-9]{8,}$/i, "")
    .replace(/[-_]+$/g, "")
    .replace(/[-_]+/g, " ")
    .trim();

  return title ? title[0].toUpperCase() + title.slice(1) : "";
}

function build(
  url: URL,
  attrs: Omit<LinearAttrs, "provider" | "url">,
): LinearAttrs {
  return { provider: "linear", url: url.toString(), ...attrs };
}

function titleFromPathSegments(values: string[]): string {
  return values.map(titleFromSlug).filter(Boolean).join(" / ");
}

function normalizeKnownKind(
  value: string | undefined,
): Exclude<LinearLinkKind, "route" | "workspace"> | null {
  switch (value) {
    case "document":
    case "issue":
    case "initiative":
    case "project":
    case "team":
    case "view":
      return value;
    default:
      return null;
  }
}

export function parseLinearUrl(rawUrl: string): LinearAttrs | null {
  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return null;
  }

  if (!HOSTS.has(url.hostname.toLowerCase())) {
    return null;
  }

  const segments = url.pathname.split("/").filter(Boolean);
  const [workspaceSegment, kindSegment, resourceSegment, detailSegment] =
    segments;
  const workspace = decodePathSegment(workspaceSegment);

  if (!workspace) {
    return null;
  }

  if (!kindSegment) {
    return build(url, {
      kind: "workspace",
      workspace,
    });
  }

  const kind = normalizeKnownKind(kindSegment);
  if (!kind) {
    const routeSegments = segments.slice(1);

    return build(url, {
      kind: "route",
      workspace,
      resourceId: routeSegments.map(decodePathSegment).join("/") || undefined,
      resourceTitle: titleFromPathSegments(routeSegments) || undefined,
    });
  }

  const titleSegment = kind === "issue" ? detailSegment : resourceSegment;

  return build(url, {
    kind,
    workspace,
    resourceId: decodePathSegment(resourceSegment) || undefined,
    resourceTitle: titleFromSlug(titleSegment) || undefined,
  });
}

function getKindLabel(attrs: LinearAttrs): string {
  switch (attrs.kind) {
    case "document":
      return attrs.resourceTitle
        ? `Document: ${attrs.resourceTitle}`
        : "Document";
    case "issue":
      return attrs.resourceId ? `Issue ${attrs.resourceId}` : "Issue";
    case "initiative":
      return attrs.resourceTitle
        ? `Initiative: ${attrs.resourceTitle}`
        : "Initiative";
    case "project":
      return attrs.resourceTitle
        ? `Project: ${attrs.resourceTitle}`
        : "Project";
    case "route":
      return attrs.resourceTitle ? `Route: ${attrs.resourceTitle}` : "Route";
    case "team":
      return attrs.resourceTitle ? `Team: ${attrs.resourceTitle}` : "Team";
    case "view":
      return attrs.resourceTitle ? `View: ${attrs.resourceTitle}` : "View";
    case "workspace":
      return "Workspace";
  }
}

export function getLinearDisplayParts(attrs: LinearAttrs): {
  header: string;
  subline: string;
} {
  return {
    header: attrs.workspace ?? "Linear",
    subline: getKindLabel(attrs),
  };
}
