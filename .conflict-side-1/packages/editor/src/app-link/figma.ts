export type FigmaLinkKind =
  | "board"
  | "design"
  | "file"
  | "prototype"
  | "slides";

export interface FigmaAttrs {
  provider: "figma";
  kind: FigmaLinkKind;
  url: string;
  resourceId?: string;
  resourceTitle?: string;
}

const HOSTS = new Set(["figma.com", "www.figma.com"]);

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

  return decoded.replace(/[-_]+/g, " ").trim();
}

function build(
  url: URL,
  attrs: Omit<FigmaAttrs, "provider" | "url">,
): FigmaAttrs {
  return { provider: "figma", url: url.toString(), ...attrs };
}

function normalizeKind(value: string | undefined): FigmaLinkKind | null {
  switch (value) {
    case "board":
    case "design":
    case "file":
    case "slides":
      return value;
    case "proto":
      return "prototype";
    default:
      return null;
  }
}

export function parseFigmaUrl(rawUrl: string): FigmaAttrs | null {
  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return null;
  }

  if (!HOSTS.has(url.hostname.toLowerCase())) {
    return null;
  }

  const [kindSegment, resourceId, titleSegment] = url.pathname
    .split("/")
    .filter(Boolean);
  const kind = normalizeKind(kindSegment);

  if (!kind || !resourceId) {
    return null;
  }

  return build(url, {
    kind,
    resourceId,
    resourceTitle: titleFromSlug(titleSegment) || undefined,
  });
}

function getKindLabel(kind: FigmaLinkKind): string {
  switch (kind) {
    case "board":
      return "FigJam board";
    case "design":
    case "file":
      return "Design file";
    case "prototype":
      return "Prototype";
    case "slides":
      return "Slides";
  }
}

export function getFigmaDisplayParts(attrs: FigmaAttrs): {
  header: string;
  subline: string;
} {
  return {
    header: "Figma",
    subline: attrs.resourceTitle
      ? `${getKindLabel(attrs.kind)}: ${attrs.resourceTitle}`
      : getKindLabel(attrs.kind),
  };
}
