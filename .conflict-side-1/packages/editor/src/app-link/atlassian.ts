export type AtlassianLinkKind =
  | "confluence_page"
  | "confluence_space"
  | "jira_issue";

export interface AtlassianAttrs {
  provider: "atlassian";
  kind: AtlassianLinkKind;
  url: string;
  workspace?: string;
  resourceId?: string;
  resourceTitle?: string;
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

function titleFromSlug(value: string | undefined): string {
  return decodePathSegment(value)
    .replace(/\+/g, " ")
    .replace(/[-_]+/g, " ")
    .trim();
}

function build(
  url: URL,
  attrs: Omit<AtlassianAttrs, "provider" | "url">,
): AtlassianAttrs {
  return { provider: "atlassian", url: url.toString(), ...attrs };
}

function workspaceFromHost(hostname: string): string | undefined {
  return hostname.endsWith(".atlassian.net")
    ? hostname.replace(/\.atlassian\.net$/, "")
    : undefined;
}

export function parseAtlassianUrl(rawUrl: string): AtlassianAttrs | null {
  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return null;
  }

  const hostname = url.hostname.toLowerCase();
  const workspace = workspaceFromHost(hostname);
  const segments = url.pathname.split("/").filter(Boolean);

  if (!workspace) {
    return null;
  }

  if (segments[0] === "browse" && segments[1]) {
    return build(url, {
      kind: "jira_issue",
      workspace,
      resourceId: decodePathSegment(segments[1]),
    });
  }

  if (segments[0] !== "wiki" || segments[1] !== "spaces" || !segments[2]) {
    return null;
  }

  if (segments[3] === "pages" && segments[4]) {
    return build(url, {
      kind: "confluence_page",
      workspace,
      resourceId: decodePathSegment(segments[4]),
      resourceTitle: titleFromSlug(segments[5]) || undefined,
    });
  }

  return build(url, {
    kind: "confluence_space",
    workspace,
    resourceId: decodePathSegment(segments[2]),
  });
}

export function getAtlassianDisplayParts(attrs: AtlassianAttrs): {
  header: string;
  subline: string;
} {
  switch (attrs.kind) {
    case "jira_issue":
      return {
        header: attrs.workspace ?? "Jira",
        subline: attrs.resourceId ? `Jira ${attrs.resourceId}` : "Jira issue",
      };
    case "confluence_page":
      return {
        header: attrs.workspace ?? "Confluence",
        subline: attrs.resourceTitle
          ? `Confluence: ${attrs.resourceTitle}`
          : "Confluence page",
      };
    case "confluence_space":
      return {
        header: attrs.workspace ?? "Confluence",
        subline: attrs.resourceId
          ? `Confluence space ${attrs.resourceId}`
          : "Confluence space",
      };
  }
}
