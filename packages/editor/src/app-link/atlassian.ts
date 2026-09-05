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
