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
