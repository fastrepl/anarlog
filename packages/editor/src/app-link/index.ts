import {
  getDiscordDisplayParts,
  parseDiscordUrl,
  type DiscordAttrs,
} from "./discord";
import {
  getGitHubDisplayParts,
  parseGitHubUrl,
  type GitHubAttrs,
} from "./github";
import { getSlackDisplayParts, parseSlackUrl, type SlackAttrs } from "./slack";

export type { GitHubAttrs, GitHubLinkKind as AppLinkKind } from "./github";
export type { SlackAttrs } from "./slack";
export type { DiscordAttrs } from "./discord";

export type AppLinkAttrs = GitHubAttrs | SlackAttrs | DiscordAttrs;

export function parseAppLinkUrl(rawUrl: string): AppLinkAttrs | null {
  const trimmed = rawUrl.trim();
  if (!trimmed) {
    return null;
  }

  return (
    parseGitHubUrl(trimmed) ??
    parseSlackUrl(trimmed) ??
    parseDiscordUrl(trimmed)
  );
}

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
  }
}

export function getAppLinkLabel(attrs: AppLinkAttrs): string {
  const { header, subline } = getAppLinkDisplayParts(attrs);
  return `${header} ${subline}`;
}
