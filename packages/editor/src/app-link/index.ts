import { getAtlassianDisplayParts, type AtlassianAttrs } from "./atlassian";
import { getDiscordDisplayParts, type DiscordAttrs } from "./discord";
import { getFigmaDisplayParts, type FigmaAttrs } from "./figma";
import { getGitHubDisplayParts, type GitHubAttrs } from "./github";
import { getGoogleDisplayParts, type GoogleAttrs } from "./google";
import { getLinearDisplayParts, type LinearAttrs } from "./linear";
import { getNotionDisplayParts, type NotionAttrs } from "./notion";
import { getSlackDisplayParts, type SlackAttrs } from "./slack";
import { getWorkDisplayParts, type WorkAttrs } from "./work";

export type { GitHubAttrs, GitHubLinkKind as AppLinkKind } from "./github";
export type { SlackAttrs } from "./slack";
export type { DiscordAttrs } from "./discord";
export type { LinearAttrs } from "./linear";
export type { NotionAttrs } from "./notion";
export type { GoogleAttrs } from "./google";
export type { FigmaAttrs } from "./figma";
export type { AtlassianAttrs } from "./atlassian";
export type { WorkAttrs } from "./work";

export type AppLinkAttrs =
  | GitHubAttrs
  | SlackAttrs
  | DiscordAttrs
  | LinearAttrs
  | NotionAttrs
  | GoogleAttrs
  | FigmaAttrs
  | AtlassianAttrs
  | WorkAttrs;

export function getAppLinkDisplayParts(attrs: AppLinkAttrs): {
  header: string;
  subline: string;
} {
  switch (attrs.provider) {
    case "github":
      return getGitHubDisplayParts(attrs);
    case "slack":
      return getSlackDisplayParts(attrs);
    case "discord":
      return getDiscordDisplayParts(attrs);
    case "linear":
      return getLinearDisplayParts(attrs);
    case "notion":
      return getNotionDisplayParts(attrs);
    case "google":
      return getGoogleDisplayParts(attrs);
    case "figma":
      return getFigmaDisplayParts(attrs);
    case "atlassian":
      return getAtlassianDisplayParts(attrs);
    case "airtable":
    case "asana":
    case "calendly":
    case "dropbox":
    case "loom":
    case "miro":
    case "trello":
    case "zoom":
      return getWorkDisplayParts(attrs);
  }
}

export function getAppLinkLabel(attrs: AppLinkAttrs): string {
  const { header, subline } = getAppLinkDisplayParts(attrs);
  return `${header} ${subline}`;
}
