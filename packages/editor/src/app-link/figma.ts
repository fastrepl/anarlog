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
