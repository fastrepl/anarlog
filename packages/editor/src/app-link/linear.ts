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
