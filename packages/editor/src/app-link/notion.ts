export type NotionLinkKind = "database" | "page" | "workspace";

export interface NotionAttrs {
  provider: "notion";
  kind: NotionLinkKind;
  url: string;
  workspace?: string;
  resourceTitle?: string;
  resourceId?: string;
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
