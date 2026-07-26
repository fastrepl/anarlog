export type GoogleLinkKind =
  | "document"
  | "file"
  | "folder"
  | "form"
  | "presentation"
  | "spreadsheet";

export interface GoogleAttrs {
  provider: "google";
  kind: GoogleLinkKind;
  url: string;
  resourceId?: string;
}

function build(
  url: URL,
  attrs: Omit<GoogleAttrs, "provider" | "url">,
): GoogleAttrs {
  return { provider: "google", url: url.toString(), ...attrs };
}

function resourceIdAfter(segments: string[], marker: string): string | null {
  const index = segments.indexOf(marker);
  return index >= 0 && segments[index + 1] ? segments[index + 1] : null;
}

export function parseGoogleUrl(rawUrl: string): GoogleAttrs | null {
  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return null;
  }

  const hostname = url.hostname.toLowerCase();
  const segments = url.pathname.split("/").filter(Boolean);

  if (hostname === "docs.google.com") {
    const [kindSegment] = segments;

    switch (kindSegment) {
      case "document": {
        const resourceId = resourceIdAfter(segments, "d");
        if (!resourceId) {
          return null;
        }
        return build(url, { kind: "document", resourceId });
      }
      case "spreadsheets": {
        const resourceId = resourceIdAfter(segments, "d");
        if (!resourceId) {
          return null;
        }
        return build(url, { kind: "spreadsheet", resourceId });
      }
      case "presentation": {
        const resourceId = resourceIdAfter(segments, "d");
        if (!resourceId) {
          return null;
        }
        return build(url, { kind: "presentation", resourceId });
      }
      case "forms": {
        const resourceId =
          segments[1] === "d" && segments[2] === "e"
            ? segments[3]
            : resourceIdAfter(segments, "d");
        if (!resourceId) {
          return null;
        }
        return build(url, { kind: "form", resourceId });
      }
    }
  }

  if (hostname === "drive.google.com") {
    const [first, second, third] = segments;

    if (first === "file" && second === "d" && third) {
      return build(url, { kind: "file", resourceId: third });
    }

    if (first === "drive" && second === "folders" && third) {
      return build(url, { kind: "folder", resourceId: third });
    }

    if (first === "open" || first === "uc") {
      const resourceId = url.searchParams.get("id") ?? undefined;
      if (resourceId) {
        return build(url, { kind: "file", resourceId });
      }
    }
  }

  return null;
}

export function getGoogleDisplayParts(attrs: GoogleAttrs): {
  header: string;
  subline: string;
} {
  switch (attrs.kind) {
    case "document":
      return { header: "Google Docs", subline: "Document" };
    case "spreadsheet":
      return { header: "Google Sheets", subline: "Spreadsheet" };
    case "presentation":
      return { header: "Google Slides", subline: "Presentation" };
    case "form":
      return { header: "Google Forms", subline: "Form" };
    case "folder":
      return { header: "Google Drive", subline: "Folder" };
    case "file":
      return { header: "Google Drive", subline: "File" };
  }
}
