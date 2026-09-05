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
