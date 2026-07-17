export type WorkProvider =
  | "airtable"
  | "asana"
  | "calendly"
  | "dropbox"
  | "loom"
  | "miro"
  | "trello"
  | "zoom";

export type WorkLinkKind =
  | "base"
  | "board"
  | "card"
  | "event"
  | "file"
  | "folder"
  | "meeting"
  | "project"
  | "recording"
  | "share"
  | "table"
  | "task"
  | "video"
  | "view";

export interface WorkAttrs {
  provider: WorkProvider;
  kind: WorkLinkKind;
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
    .replace(/\.[a-z0-9]+$/i, "")
    .replace(/[-_+]+/g, " ")
    .trim();
}

function build(url: URL, attrs: Omit<WorkAttrs, "url">): WorkAttrs {
  return { url: url.toString(), ...attrs };
}

function parseAsanaUrl(url: URL): WorkAttrs | null {
  if (url.hostname.toLowerCase() !== "app.asana.com") {
    return null;
  }

  const segments = url.pathname.split("/").filter(Boolean);

  if (segments[0] === "0" && segments[1] && segments[2]) {
    return build(url, {
      provider: "asana",
      kind: "task",
      resourceId: decodePathSegment(segments[2]),
      workspace: decodePathSegment(segments[1]),
    });
  }

  const taskIndex = segments.indexOf("task");
  if (taskIndex >= 0 && segments[taskIndex + 1]) {
    return build(url, {
      provider: "asana",
      kind: "task",
      resourceId: decodePathSegment(segments[taskIndex + 1]),
    });
  }

  const projectIndex = segments.indexOf("project");
  if (projectIndex >= 0 && segments[projectIndex + 1]) {
    return build(url, {
      provider: "asana",
      kind: "project",
      resourceId: decodePathSegment(segments[projectIndex + 1]),
    });
  }

  return null;
}

function parseTrelloUrl(url: URL): WorkAttrs | null {
  const hostname = url.hostname.toLowerCase();
  if (hostname !== "trello.com" && hostname !== "www.trello.com") {
    return null;
  }

  const [kindSegment, resourceId, titleSegment] = url.pathname
    .split("/")
    .filter(Boolean);

  if (kindSegment === "c" && resourceId) {
    return build(url, {
      provider: "trello",
      kind: "card",
      resourceId,
      resourceTitle: titleFromSlug(titleSegment) || undefined,
    });
  }

  if (kindSegment === "b" && resourceId) {
    return build(url, {
      provider: "trello",
      kind: "board",
      resourceId,
      resourceTitle: titleFromSlug(titleSegment) || undefined,
    });
  }

  return null;
}

function parseAirtableUrl(url: URL): WorkAttrs | null {
  const hostname = url.hostname.toLowerCase();
  if (hostname !== "airtable.com" && hostname !== "www.airtable.com") {
    return null;
  }

  const [first, second, third] = url.pathname.split("/").filter(Boolean);

  if (!first) {
    return null;
  }

  if (first.startsWith("shr")) {
    return build(url, {
      provider: "airtable",
      kind: "share",
      resourceId: first,
    });
  }

  if (!first.startsWith("app")) {
    return null;
  }

  if (third?.startsWith("viw")) {
    return build(url, {
      provider: "airtable",
      kind: "view",
      resourceId: third,
      workspace: first,
    });
  }

  if (second?.startsWith("tbl")) {
    return build(url, {
      provider: "airtable",
      kind: "table",
      resourceId: second,
      workspace: first,
    });
  }

  return build(url, {
    provider: "airtable",
    kind: "base",
    resourceId: first,
  });
}

function parseMiroUrl(url: URL): WorkAttrs | null {
  const hostname = url.hostname.toLowerCase();
  if (hostname !== "miro.com" && hostname !== "www.miro.com") {
    return null;
  }

  const segments = url.pathname.split("/").filter(Boolean);
  const boardIndex = segments.indexOf("board");
  const resourceId = boardIndex >= 0 ? segments[boardIndex + 1] : undefined;

  if (!resourceId) {
    return null;
  }

  return build(url, {
    provider: "miro",
    kind: "board",
    resourceId: decodePathSegment(resourceId),
  });
}

function parseLoomUrl(url: URL): WorkAttrs | null {
  const hostname = url.hostname.toLowerCase();
  if (hostname !== "loom.com" && hostname !== "www.loom.com") {
    return null;
  }

  const [kindSegment, resourceId] = url.pathname.split("/").filter(Boolean);
  if ((kindSegment === "share" || kindSegment === "embed") && resourceId) {
    return build(url, {
      provider: "loom",
      kind: "video",
      resourceId,
    });
  }

  return null;
}

function parseDropboxUrl(url: URL): WorkAttrs | null {
  const hostname = url.hostname.toLowerCase();
  if (hostname !== "dropbox.com" && hostname !== "www.dropbox.com") {
    return null;
  }

  const segments = url.pathname.split("/").filter(Boolean);
  const [first, second, third, fourth] = segments;

  if (first === "s" && second) {
    return build(url, {
      provider: "dropbox",
      kind: "file",
      resourceId: second,
      resourceTitle: titleFromSlug(third) || undefined,
    });
  }

  if (first === "scl" && second === "fi" && third) {
    return build(url, {
      provider: "dropbox",
      kind: "file",
      resourceId: third,
      resourceTitle: titleFromSlug(fourth) || undefined,
    });
  }

  if (first === "scl" && second === "fo" && third) {
    return build(url, {
      provider: "dropbox",
      kind: "folder",
      resourceId: third,
    });
  }

  return null;
}

function parseZoomUrl(url: URL): WorkAttrs | null {
  const hostname = url.hostname.toLowerCase();
  if (hostname !== "zoom.us" && !hostname.endsWith(".zoom.us")) {
    return null;
  }

  const segments = url.pathname.split("/").filter(Boolean);

  if (segments[0] === "j" && segments[1]) {
    return build(url, {
      provider: "zoom",
      kind: "meeting",
      resourceId: segments[1],
      workspace: hostname.replace(/\.zoom\.us$/, ""),
    });
  }

  if (segments[0] === "rec" && segments[1] === "share" && segments[2]) {
    return build(url, {
      provider: "zoom",
      kind: "recording",
      resourceId: segments[2],
      workspace: hostname.replace(/\.zoom\.us$/, ""),
    });
  }

  return null;
}

function parseCalendlyUrl(url: URL): WorkAttrs | null {
  const hostname = url.hostname.toLowerCase();
  if (hostname !== "calendly.com" && hostname !== "www.calendly.com") {
    return null;
  }

  const [userSegment, eventSegment] = url.pathname.split("/").filter(Boolean);
  const user = decodePathSegment(userSegment);

  if (!user || !eventSegment) {
    return null;
  }

  return build(url, {
    provider: "calendly",
    kind: "event",
    workspace: user,
    resourceId: decodePathSegment(eventSegment),
    resourceTitle: titleFromSlug(eventSegment) || undefined,
  });
}

export function parseWorkUrl(rawUrl: string): WorkAttrs | null {
  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return null;
  }

  return (
    parseAsanaUrl(url) ??
    parseTrelloUrl(url) ??
    parseAirtableUrl(url) ??
    parseMiroUrl(url) ??
    parseLoomUrl(url) ??
    parseDropboxUrl(url) ??
    parseZoomUrl(url) ??
    parseCalendlyUrl(url)
  );
}

function getProviderLabel(provider: WorkProvider): string {
  switch (provider) {
    case "airtable":
      return "Airtable";
    case "asana":
      return "Asana";
    case "calendly":
      return "Calendly";
    case "dropbox":
      return "Dropbox";
    case "loom":
      return "Loom";
    case "miro":
      return "Miro";
    case "trello":
      return "Trello";
    case "zoom":
      return "Zoom";
  }
}

function getKindLabel(attrs: WorkAttrs): string {
  switch (attrs.kind) {
    case "base":
      return "Base";
    case "board":
      return attrs.resourceTitle ? `Board: ${attrs.resourceTitle}` : "Board";
    case "card":
      return attrs.resourceTitle ? `Card: ${attrs.resourceTitle}` : "Card";
    case "event":
      return attrs.resourceTitle ? `Event: ${attrs.resourceTitle}` : "Event";
    case "file":
      return attrs.resourceTitle ? `File: ${attrs.resourceTitle}` : "File";
    case "folder":
      return "Folder";
    case "meeting":
      return attrs.resourceId ? `Meeting ${attrs.resourceId}` : "Meeting";
    case "project":
      return attrs.resourceId ? `Project ${attrs.resourceId}` : "Project";
    case "recording":
      return "Recording";
    case "share":
      return "Shared view";
    case "table":
      return "Table";
    case "task":
      return attrs.resourceId ? `Task ${attrs.resourceId}` : "Task";
    case "video":
      return "Video";
    case "view":
      return "View";
  }
}

export function getWorkDisplayParts(attrs: WorkAttrs): {
  header: string;
  subline: string;
} {
  return {
    header: getProviderLabel(attrs.provider),
    subline: getKindLabel(attrs),
  };
}
