export type NotionLinkKind = "database" | "page" | "workspace";

export interface NotionAttrs {
  provider: "notion";
  kind: NotionLinkKind;
  url: string;
  workspace?: string;
  resourceTitle?: string;
  resourceId?: string;
}

const HOSTS = new Set(["notion.so", "www.notion.so", "notion.site"]);

function isNotionHost(hostname: string): boolean {
  const normalized = hostname.toLowerCase();
  return HOSTS.has(normalized) || normalized.endsWith(".notion.site");
}

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

function parseNotionResource(value: string | undefined): {
  id?: string;
  title?: string;
} {
  const decoded = decodePathSegment(value);
  if (!decoded) {
    return {};
  }

  const idMatch = decoded.match(/[a-f0-9]{32}$/i);
  const id = idMatch?.[0];
  const title = decoded
    .replace(/[a-f0-9]{32}$/i, "")
    .replace(/[-_]+$/g, "")
    .replace(/[-_]+/g, " ")
    .trim();

  return {
    id,
    title: title || undefined,
  };
}

function build(
  url: URL,
  attrs: Omit<NotionAttrs, "provider" | "url">,
): NotionAttrs {
  return { provider: "notion", url: url.toString(), ...attrs };
}

export function parseNotionUrl(rawUrl: string): NotionAttrs | null {
  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return null;
  }

  const hostname = url.hostname.toLowerCase();
  if (!isNotionHost(hostname)) {
    return null;
  }

  const segments = url.pathname.split("/").filter(Boolean);
  const workspace =
    hostname.endsWith(".notion.site") && hostname !== "notion.site"
      ? hostname.replace(/\.notion\.site$/, "")
      : undefined;

  if (segments.length === 0) {
    return build(url, { kind: "workspace", workspace });
  }

  const databaseIndex = segments.findIndex((segment) => segment === "database");
  const resourceSegment =
    databaseIndex >= 0
      ? segments[databaseIndex + 1]
      : segments[segments.length - 1];
  const resource = parseNotionResource(resourceSegment);
  const kind = databaseIndex >= 0 ? "database" : "page";

  return build(url, {
    kind,
    workspace,
    resourceId: resource.id,
    resourceTitle: resource.title,
  });
}

export function getNotionDisplayParts(attrs: NotionAttrs): {
  header: string;
  subline: string;
} {
  if (attrs.kind === "workspace") {
    return {
      header: attrs.workspace ?? "Notion",
      subline: "Workspace",
    };
  }

  const label = attrs.kind === "database" ? "Database" : "Page";
  return {
    header: attrs.workspace ?? "Notion",
    subline: attrs.resourceTitle ? `${label}: ${attrs.resourceTitle}` : label,
  };
}
