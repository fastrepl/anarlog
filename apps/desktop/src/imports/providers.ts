import type { InstalledApp } from "@anlg/plugin-detect";

export type MeetingImportProvider = {
  id: string;
  name: string;
  access: "API" | "CLI" | "Export" | "MCP" | "OAuth" | "Webhook";
  helpUrl: string;
  directImport?: "cli" | "mcp-oauth" | "nango-oauth";
  nangoIntegrationId?: string;
  nativeNames?: string[];
  bundleIds?: string[];
  alwaysAvailable?: boolean;
};

export type DetectedMeetingImportProvider = MeetingImportProvider & {
  installedAppId: string;
  iconUrl?: string;
};

export const MEETING_IMPORT_PROVIDERS: MeetingImportProvider[] = [
  {
    id: "granola",
    name: "Granola",
    access: "MCP",
    helpUrl: "https://docs.granola.ai/help-center/sharing/integrations/mcp",
    directImport: "mcp-oauth",
    nativeNames: ["Granola"],
    bundleIds: ["com.granola.app", "com.getgranola.app"],
  },
  {
    id: "circleback",
    name: "Circleback",
    access: "MCP",
    helpUrl:
      "https://support.circleback.ai/en/articles/13249081-circleback-mcp",
    directImport: "mcp-oauth",
    nativeNames: ["Circleback"],
  },
  {
    id: "fireflies",
    name: "Fireflies.ai",
    access: "MCP",
    helpUrl: "https://docs.fireflies.ai/mcp-tools/overview",
    directImport: "mcp-oauth",
    nativeNames: ["Fireflies", "Fireflies.ai"],
  },
  {
    id: "krisp",
    name: "Krisp",
    access: "MCP",
    helpUrl: "https://help.krisp.ai/hc/en-us/articles/25396920405148-Krisp-MCP",
    directImport: "mcp-oauth",
    nativeNames: ["Krisp"],
    bundleIds: ["ai.krisp.krispMac"],
  },
  {
    id: "fathom",
    name: "Fathom",
    access: "OAuth",
    helpUrl: "https://developers.fathom.ai/sdks/oauth",
    directImport: "nango-oauth",
    nangoIntegrationId: "fathom",
    nativeNames: ["Fathom"],
    bundleIds: ["Fathom"],
  },
  {
    id: "read-ai",
    name: "Read AI",
    access: "MCP",
    helpUrl:
      "https://support.read.ai/hc/en-us/articles/49379985941523-Read-AI-API-and-MCP-Overview",
    directImport: "mcp-oauth",
    nativeNames: ["Read AI"],
  },
  {
    id: "notion",
    name: "Notion AI Meeting Notes",
    access: "OAuth",
    helpUrl: "https://developers.notion.com/reference/query-meeting-notes",
    directImport: "nango-oauth",
    nangoIntegrationId: "notion",
    nativeNames: ["Notion"],
    bundleIds: ["notion.id", "notion"],
  },
  {
    id: "fellow",
    name: "Fellow",
    access: "MCP",
    helpUrl: "https://help.fellow.ai/en/articles/12622641-fellow-s-mcp-server",
    directImport: "mcp-oauth",
    nativeNames: ["Fellow"],
  },
  {
    id: "tactiq",
    name: "Tactiq",
    access: "MCP",
    helpUrl:
      "https://help.tactiq.io/en/articles/14883619-connecting-tactiq-mcp-server",
    directImport: "mcp-oauth",
  },
  {
    id: "grain",
    name: "Grain",
    access: "API",
    helpUrl: "https://developers.grain.com/",
    nativeNames: ["Grain"],
  },
  {
    id: "otter",
    name: "Otter.ai",
    access: "API",
    helpUrl:
      "https://help.otter.ai/hc/en-us/articles/36130822688279-Otter-ai-Public-API",
    nativeNames: ["Otter", "Otter.ai"],
  },
  {
    id: "tldv",
    name: "tl;dv",
    access: "API",
    helpUrl: "https://intercom.help/tldv/en/articles/11583137-api",
    nativeNames: ["tl;dv", "tldv"],
  },
  {
    id: "meetgeek",
    name: "MeetGeek",
    access: "API",
    helpUrl: "https://docs.meetgeek.ai/api/getting-started/authorization",
    nativeNames: ["MeetGeek"],
  },
  {
    id: "avoma",
    name: "Avoma",
    access: "API",
    helpUrl: "https://dev.avoma.com/",
  },
  {
    id: "gong",
    name: "Gong",
    access: "API",
    helpUrl: "https://help.gong.io/v1/docs/what-the-gong-api-provides",
  },
  {
    id: "clari-copilot",
    name: "Clari Copilot",
    access: "API",
    helpUrl: "https://api-doc.copilot.clari.com/",
    nativeNames: ["Clari Copilot", "Wingman"],
  },
  {
    id: "jiminny",
    name: "Jiminny",
    access: "MCP",
    helpUrl: "https://help.jiminny.com/en/articles/15292810-jiminny-mcp",
    directImport: "mcp-oauth",
    nativeNames: ["Jiminny", "Jiminny Sidekick"],
  },
  {
    id: "supernormal",
    name: "Supernormal",
    access: "API",
    helpUrl: "https://docs.supernormal.com/api-reference/introduction",
    nativeNames: ["Supernormal"],
  },
  {
    id: "plaud",
    name: "Plaud",
    access: "CLI",
    helpUrl:
      "https://support.plaud.ai/hc/en-us/articles/57751026815257-Plaud-CLI",
    directImport: "cli",
    nativeNames: ["Plaud", "Plaud Desktop"],
    bundleIds: ["ai.plaud.desktop.plaud"],
  },
  {
    id: "pocket",
    name: "Pocket",
    access: "MCP",
    helpUrl: "https://docs.heypocketai.com/docs",
    directImport: "mcp-oauth",
    nativeNames: ["Pocket", "Pocket Desktop", "Pocket AI"],
    bundleIds: ["com.openvisionengineering.pocket-desktop-app"],
  },
  {
    id: "zoom",
    name: "Zoom",
    access: "OAuth",
    helpUrl: "https://developers.zoom.us/docs/api/meetings/",
    directImport: "nango-oauth",
    nangoIntegrationId: "zoom",
    nativeNames: ["zoom.us", "Zoom", "Zoom Workplace"],
    bundleIds: ["us.zoom.xos"],
  },
  {
    id: "microsoft-teams",
    name: "Microsoft Teams",
    access: "OAuth",
    helpUrl:
      "https://learn.microsoft.com/en-us/graph/api/onlinemeeting-list-transcripts?view=graph-rest-1.0",
    directImport: "nango-oauth",
    nangoIntegrationId: "microsoft-teams",
    nativeNames: ["Microsoft Teams", "Microsoft Teams (work or school)"],
    bundleIds: ["com.microsoft.teams", "com.microsoft.teams2"],
  },
  {
    id: "google-meet",
    name: "Google Meet",
    access: "OAuth",
    helpUrl:
      "https://developers.google.com/workspace/meet/api/guides/artifacts",
    directImport: "nango-oauth",
    nangoIntegrationId: "google-meet",
    nativeNames: ["Google Meet"],
    alwaysAvailable: true,
  },
  {
    id: "webex",
    name: "Webex",
    access: "OAuth",
    helpUrl:
      "https://developer.webex.com/meeting/docs/api/v1/meeting-transcripts",
    directImport: "nango-oauth",
    nangoIntegrationId: "webex",
    nativeNames: ["Webex", "Cisco Webex Meetings"],
    bundleIds: ["com.cisco.webex", "com.webex"],
  },
  {
    id: "sembly",
    name: "Sembly",
    access: "Webhook",
    helpUrl:
      "https://helpdesk.sembly.ai/hc/en-us/articles/17664440116369-Guide-to-Outbound-Integrations-in-Sembly-for-API-Integrators-and-Custom-Adapter-Developers",
    nativeNames: ["Sembly"],
  },
  {
    id: "notta",
    name: "Notta",
    access: "Export",
    helpUrl:
      "https://support.notta.ai/hc/en-us/articles/18646448413083-Download-and-Share-transcripts",
    nativeNames: ["Notta"],
  },
  {
    id: "bluedot",
    name: "Bluedot",
    access: "Export",
    helpUrl:
      "https://help.bluedothq.com/en/articles/12497819-export-transcript-as-pdf-or-txt",
  },
  {
    id: "jamie",
    name: "Jamie",
    access: "Export",
    helpUrl: "https://www.meetjamie.ai/",
    nativeNames: ["Jamie"],
    bundleIds: ["com.jamie.app"],
  },
  {
    id: "chatgpt-record",
    name: "ChatGPT Record",
    access: "Export",
    helpUrl:
      "https://help.openai.com/en/articles/7260999-how-do-i-export-my-chatgpt-history-and-data.csv",
    nativeNames: ["ChatGPT"],
    bundleIds: ["com.openai.chat", "com.openai.chatgpt"],
  },
  {
    id: "slack-huddles",
    name: "Slack Huddles",
    access: "Export",
    helpUrl:
      "https://slack.com/help/articles/31377193680019-Use-AI-to-take-huddle-notes-in-Slack",
    nativeNames: ["Slack"],
    bundleIds: ["com.tinyspeck.slackmacgap"],
  },
  {
    id: "limitless",
    name: "Limitless / Rewind",
    access: "Export",
    helpUrl: "https://developers.limitless.ai/",
    nativeNames: ["Limitless", "Rewind"],
    bundleIds: ["ai.limitless.desktop", "com.memoryvault.MemoryVault"],
  },
];

const normalize = (value: string) =>
  value
    .trim()
    .toLowerCase()
    .replace(/\.app$/u, "")
    .replace(/[^a-z0-9]+/gu, " ")
    .trim();

export function detectMeetingImportProviders(
  installedApps: InstalledApp[],
): DetectedMeetingImportProvider[] {
  const installed = installedApps.map((app) => ({
    id: app.id,
    normalizedId: app.id.toLowerCase(),
    name: normalize(app.name),
  }));

  return MEETING_IMPORT_PROVIDERS.flatMap((provider) => {
    const installedApp = installed.find(
      (app) =>
        provider.nativeNames?.some((name) => app.name === normalize(name)) ||
        provider.bundleIds?.some(
          (bundleId) => app.normalizedId === bundleId.toLowerCase(),
        ),
    );

    return installedApp
      ? [{ ...provider, installedAppId: installedApp.id }]
      : provider.alwaysAvailable
        ? [{ ...provider, installedAppId: provider.id }]
        : [];
  });
}
