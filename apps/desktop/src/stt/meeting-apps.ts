import { resolveResource } from "@tauri-apps/api/path";

import type { NotificationIcon } from "@anlg/plugin-notification";

import type { NearbyCalendarEvent } from "~/calendar/queries";

export const BROWSER_AUTO_STOP_APP_IDS = new Set([
  "at.studio.AsideBrowser",
  "app.zen-browser.zen",
  "ai.perplexity.comet",
  "com.apple.Safari",
  "com.apple.SafariTechnologyPreview",
  "com.browseros.BrowserOS",
  "com.brave.Browser",
  "com.brave.Browser.beta",
  "com.brave.Browser.nightly",
  "com.duckduckgo.macos.browser",
  "chrome",
  "chromium",
  "chromium-browser",
  "com.google.Chrome",
  "com.google.Chrome.canary",
  "firefox",
  "google-chrome",
  "google-chrome-beta",
  "google-chrome-stable",
  "microsoft-edge",
  "msedge",
  "com.kagi.kagimacOS",
  "com.kagi.kagimacOS.RC",
  "com.microsoft.edgemac",
  "com.microsoft.edgemac.Beta",
  "com.microsoft.edgemac.Canary",
  "com.microsoft.edgemac.Dev",
  "com.operasoftware.Opera",
  "com.operasoftware.OperaDeveloper",
  "com.operasoftware.OperaGX",
  "com.operasoftware.OperaNext",
  "com.nousresearch.hermes",
  "com.sigmaos.sigmaos.macos",
  "com.vivaldi.Vivaldi",
  "company.thebrowser.Browser",
  "company.thebrowser.dia",
  "net.imput.helium",
  "net.mullvad.mullvadbrowser",
  "net.waterfox.waterfox",
  "org.chromium.Chromium",
  "org.mozilla.firefox",
  "org.mozilla.firefoxdeveloperedition",
  "org.mozilla.librewolf",
  "org.mozilla.nightly",
  "org.torproject.torbrowser",
]);

export type MicApp = { id: string; name: string };
type NearbyEvent = NearbyCalendarEvent;
export type MeetingPlatform = {
  displayName: string;
  iconResource: NotificationIconResource;
};

const NOTIFICATION_ICON_RESOURCES = {
  calCom: "notification-icons/cal-com.png",
  calVideo: "notification-icons/cal-video.png",
  daily: "notification-icons/daily.png",
  discord: "notification-icons/discord.png",
  googleMeet: "notification-icons/google-meet.svg",
  gotomeeting: "notification-icons/gotomeeting.png",
  jitsi: "notification-icons/jitsi.png",
  kakaotalk: "notification-icons/kakaotalk.png",
  line: "notification-icons/line.png",
  messenger: "notification-icons/messenger.png",
  microsoftTeams: "notification-icons/microsoft-teams.svg",
  phone: "notification-icons/phone.png",
  signal: "notification-icons/signal.png",
  slack: "notification-icons/slack.svg",
  telegram: "notification-icons/telegram.png",
  webex: "notification-icons/webex.svg",
  whatsapp: "notification-icons/whatsapp.png",
  whereby: "notification-icons/whereby.png",
  zoom: "notification-icons/zoom.svg",
} as const;

type NotificationIconResource = keyof typeof NOTIFICATION_ICON_RESOURCES;

const notificationIconResourceCache = new Map<
  NotificationIconResource,
  Promise<NotificationIcon | null>
>();

const BROWSER_MEETING_ICON: NotificationIcon = {
  type: "system_symbol",
  name: "video.fill",
};

const MEETING_PLATFORMS = {
  zoom: {
    displayName: "Zoom",
    iconResource: "zoom",
  },
  googleMeet: {
    displayName: "Google Meet",
    iconResource: "googleMeet",
  },
  webex: {
    displayName: "Webex",
    iconResource: "webex",
  },
  teams: {
    displayName: "Microsoft Teams",
    iconResource: "microsoftTeams",
  },
  calCom: {
    displayName: "Cal.com",
    iconResource: "calCom",
  },
  calVideo: {
    displayName: "Cal Video",
    iconResource: "calVideo",
  },
  daily: {
    displayName: "Daily",
    iconResource: "daily",
  },
  whereby: {
    displayName: "Whereby",
    iconResource: "whereby",
  },
  jitsi: {
    displayName: "Jitsi",
    iconResource: "jitsi",
  },
  gotomeeting: {
    displayName: "GoTo Meeting",
    iconResource: "gotomeeting",
  },
  slack: {
    displayName: "Slack",
    iconResource: "slack",
  },
  discord: {
    displayName: "Discord",
    iconResource: "discord",
  },
  whatsapp: {
    displayName: "WhatsApp",
    iconResource: "whatsapp",
  },
  kakaotalk: {
    displayName: "KakaoTalk",
    iconResource: "kakaotalk",
  },
  telegram: {
    displayName: "Telegram",
    iconResource: "telegram",
  },
  signal: {
    displayName: "Signal",
    iconResource: "signal",
  },
  line: {
    displayName: "LINE",
    iconResource: "line",
  },
  messenger: {
    displayName: "Messenger",
    iconResource: "messenger",
  },
} satisfies Record<string, MeetingPlatform>;

type MicAppNotificationOverride = {
  ids: Set<string>;
  names: Set<string>;
  displayName: string;
  meetingPlatform?: MeetingPlatform;
  icon?: NotificationIcon;
  iconResource?: NotificationIconResource;
};

const MIC_APP_NOTIFICATION_OVERRIDES = [
  {
    ids: new Set([
      "/usr/libexec/avconferenced",
      "com.apple.avconferenced",
      "com.apple.TelephonyUtilities",
      "com.apple.TelephonyUtilities.callservicesd",
    ]),
    names: new Set(["av capture", "avcapture", "avconferenced", "iphone call"]),
    displayName: "iPhone Call",
    iconResource: "phone",
  },
  {
    ids: new Set(["com.apple.FaceTime"]),
    names: new Set(["facetime"]),
    displayName: "FaceTime",
    icon: {
      type: "bundle_id",
      bundle_id: "com.apple.FaceTime",
    } satisfies NotificationIcon,
  },
  {
    ids: new Set(["us.zoom.xos", "zoom", "Zoom.exe"]),
    names: new Set(["zoom", "zoom helper", "zoom workplace"]),
    displayName: "Zoom",
    meetingPlatform: MEETING_PLATFORMS.zoom,
    iconResource: "zoom",
  },
  {
    ids: new Set([
      "com.microsoft.teams",
      "com.microsoft.teams2",
      "ms-teams",
      "teams",
      "teams-for-linux",
    ]),
    names: new Set([
      "microsoft teams",
      "microsoft teams helper",
      "teams",
      "teams helper",
    ]),
    displayName: "Microsoft Teams",
    meetingPlatform: MEETING_PLATFORMS.teams,
    iconResource: "microsoftTeams",
  },
  {
    ids: new Set([
      "Cisco-Systems.Spark",
      "com.cisco.webex",
      "com.cisco.webexmeetingsapp",
    ]),
    names: new Set(["cisco webex", "webex", "webex helper", "webex meetings"]),
    displayName: "Webex",
    meetingPlatform: MEETING_PLATFORMS.webex,
    iconResource: "webex",
  },
  {
    ids: new Set([
      "com.slack.Slack",
      "com.tinyspeck.slackmacgap",
      "slack",
      "slack-desktop",
      "Slack.exe",
    ]),
    names: new Set(["slack", "slack helper"]),
    displayName: "Slack",
    meetingPlatform: MEETING_PLATFORMS.slack,
    iconResource: "slack",
  },
  {
    ids: new Set(["com.kakao.KakaoTalkMac"]),
    names: new Set(["kakaotalk", "kakaotalk helper"]),
    displayName: "KakaoTalk",
    meetingPlatform: MEETING_PLATFORMS.kakaotalk,
    iconResource: "kakaotalk",
  },
  {
    ids: new Set(["net.whatsapp.WhatsApp"]),
    names: new Set(["whatsapp", "whatsapp helper"]),
    displayName: "WhatsApp",
    meetingPlatform: MEETING_PLATFORMS.whatsapp,
    iconResource: "whatsapp",
  },
  {
    ids: new Set(["com.hnc.Discord", "com.discordapp.Discord"]),
    names: new Set(["discord", "discord helper"]),
    displayName: "Discord",
    meetingPlatform: MEETING_PLATFORMS.discord,
    iconResource: "discord",
  },
  {
    ids: new Set(["org.whispersystems.signal-desktop"]),
    names: new Set(["signal", "signal helper"]),
    displayName: "Signal",
    meetingPlatform: MEETING_PLATFORMS.signal,
    iconResource: "signal",
  },
  {
    ids: new Set(["ru.keepcoder.Telegram", "ru.keepcoder.TelegramLite"]),
    names: new Set(["telegram", "telegram helper", "telegram lite"]),
    displayName: "Telegram",
    meetingPlatform: MEETING_PLATFORMS.telegram,
    iconResource: "telegram",
  },
  {
    ids: new Set(["jp.naver.line.mac"]),
    names: new Set(["line", "line helper"]),
    displayName: "LINE",
    meetingPlatform: MEETING_PLATFORMS.line,
    iconResource: "line",
  },
  {
    ids: new Set(["com.facebook.archon"]),
    names: new Set(["messenger", "messenger helper"]),
    displayName: "Messenger",
    meetingPlatform: MEETING_PLATFORMS.messenger,
    iconResource: "messenger",
  },
] satisfies MicAppNotificationOverride[];

function getMicAppNotificationOverride(app: MicApp) {
  const normalizedName = app.name.trim().toLowerCase();
  return (
    MIC_APP_NOTIFICATION_OVERRIDES.find((override) =>
      override.names.has(normalizedName),
    ) ??
    MIC_APP_NOTIFICATION_OVERRIDES.find((override) => override.ids.has(app.id))
  );
}

function getNotificationResourceIcon(
  resource: NotificationIconResource,
): Promise<NotificationIcon | null> {
  const cached = notificationIconResourceCache.get(resource);
  if (cached) {
    return cached;
  }

  const promise = resolveResource(NOTIFICATION_ICON_RESOURCES[resource])
    .then((path): NotificationIcon => ({ type: "path", path }))
    .catch(() => null);

  notificationIconResourceCache.set(resource, promise);
  return promise;
}

async function getMeetingPlatformIcon(
  platform: MeetingPlatform,
): Promise<NotificationIcon> {
  return (
    (await getNotificationResourceIcon(platform.iconResource)) ??
    BROWSER_MEETING_ICON
  );
}

function getNotificationIconForAppId(appId: string): NotificationIcon | null {
  if (!appId || appId.startsWith("pid:")) {
    return null;
  }

  if (appId.startsWith("/") || appId.startsWith("~/")) {
    return { type: "path", path: appId };
  }

  return { type: "bundle_id", bundle_id: appId };
}

export async function getNotificationIconForApp(
  app: MicApp,
): Promise<NotificationIcon | null> {
  const override = getMicAppNotificationOverride(app);
  if (override?.iconResource) {
    const icon = await getNotificationResourceIcon(override.iconResource);
    if (icon) {
      return icon;
    }
  }

  return override?.icon ?? getNotificationIconForAppId(app.id);
}

export function getNotificationAppName(app: MicApp) {
  return getMicAppNotificationOverride(app)?.displayName ?? app.name;
}

function isBrowserApp(app: MicApp) {
  return BROWSER_AUTO_STOP_APP_IDS.has(app.id);
}

function detectMeetingPlatformFromUrl(value: string): MeetingPlatform | null {
  try {
    const parsed = new URL(value);
    const hostname = parsed.hostname.toLowerCase();
    const pathname = parsed.pathname.toLowerCase();

    if (hostname === "zoom.us" || hostname.endsWith(".zoom.us")) {
      return MEETING_PLATFORMS.zoom;
    }

    if (hostname === "meet.google.com") {
      return MEETING_PLATFORMS.googleMeet;
    }

    if (hostname === "webex.com" || hostname.endsWith(".webex.com")) {
      return MEETING_PLATFORMS.webex;
    }

    if (hostname === "teams.microsoft.com" || hostname === "teams.live.com") {
      return MEETING_PLATFORMS.teams;
    }

    if (
      hostname === "app.cal.video" ||
      hostname === "cal.video" ||
      hostname.endsWith(".cal.video") ||
      ((hostname === "cal.com" || hostname === "app.cal.com") &&
        pathname.startsWith("/video/"))
    ) {
      return MEETING_PLATFORMS.calVideo;
    }

    if (hostname === "cal.com" || hostname === "app.cal.com") {
      return MEETING_PLATFORMS.calCom;
    }

    if (hostname === "daily.co" || hostname.endsWith(".daily.co")) {
      return MEETING_PLATFORMS.daily;
    }

    if (
      hostname === "whereby.com" ||
      hostname.endsWith(".whereby.com") ||
      hostname === "appear.in" ||
      hostname.endsWith(".appear.in")
    ) {
      return MEETING_PLATFORMS.whereby;
    }

    if (hostname === "meet.jit.si" || hostname.endsWith(".jitsi.org")) {
      return MEETING_PLATFORMS.jitsi;
    }

    if (
      hostname === "gotomeeting.com" ||
      hostname.endsWith(".gotomeeting.com") ||
      hostname === "goto.com" ||
      hostname.endsWith(".goto.com")
    ) {
      return MEETING_PLATFORMS.gotomeeting;
    }

    if (hostname === "slack.com" || hostname.endsWith(".slack.com")) {
      return MEETING_PLATFORMS.slack;
    }

    if (
      hostname === "discord.com" ||
      hostname.endsWith(".discord.com") ||
      hostname === "discord.gg"
    ) {
      return MEETING_PLATFORMS.discord;
    }

    if (hostname === "web.whatsapp.com" || hostname === "whatsapp.com") {
      return MEETING_PLATFORMS.whatsapp;
    }

    if (hostname === "talk.kakao.com" || hostname.endsWith(".kakao.com")) {
      return MEETING_PLATFORMS.kakaotalk;
    }

    if (
      hostname === "web.telegram.org" ||
      hostname === "t.me" ||
      hostname === "telegram.me"
    ) {
      return MEETING_PLATFORMS.telegram;
    }

    if (hostname === "signal.me") {
      return MEETING_PLATFORMS.signal;
    }

    if (hostname === "line.me" || hostname.endsWith(".line.me")) {
      return MEETING_PLATFORMS.line;
    }

    if (hostname === "messenger.com" || hostname === "www.messenger.com") {
      return MEETING_PLATFORMS.messenger;
    }
  } catch {}

  return null;
}

function detectMeetingPlatformFromText(value: string): MeetingPlatform | null {
  const urls = value.match(/https?:\/\/[^\s<>"')]+/g) ?? [];
  for (const url of urls) {
    const platform = detectMeetingPlatformFromUrl(url);
    if (platform) {
      return platform;
    }
  }

  const normalized = value.toLowerCase();
  if (/\bgoogle meet\b/.test(normalized)) {
    return MEETING_PLATFORMS.googleMeet;
  }
  if (/\bmicrosoft teams\b/.test(normalized)) {
    return MEETING_PLATFORMS.teams;
  }
  if (/\bzoom meeting\b/.test(normalized)) {
    return MEETING_PLATFORMS.zoom;
  }
  if (/\bwebex\b/.test(normalized)) {
    return MEETING_PLATFORMS.webex;
  }
  if (
    /\bcal video\b|(^|[^a-z0-9])cal\.video([^a-z0-9]|$)|(^|[^a-z0-9])cal\.com\/video\//.test(
      normalized,
    )
  ) {
    return MEETING_PLATFORMS.calVideo;
  }
  if (/(^|[^a-z0-9])cal\.com([^a-z0-9]|$)/.test(normalized)) {
    return MEETING_PLATFORMS.calCom;
  }
  if (
    /(^|[^a-z0-9])daily\.co([^a-z0-9]|$)/.test(normalized) ||
    /\bdaily prebuilt\b/.test(normalized)
  ) {
    return MEETING_PLATFORMS.daily;
  }
  if (/\bwhereby\b/.test(normalized)) {
    return MEETING_PLATFORMS.whereby;
  }
  if (/\bjitsi\b/.test(normalized)) {
    return MEETING_PLATFORMS.jitsi;
  }
  if (/\bgoto meeting\b|\bgotomeeting\b/.test(normalized)) {
    return MEETING_PLATFORMS.gotomeeting;
  }
  if (/\bslack (huddle|call)\b/.test(normalized)) {
    return MEETING_PLATFORMS.slack;
  }
  if (/\bdiscord (call|meeting|voice)\b/.test(normalized)) {
    return MEETING_PLATFORMS.discord;
  }
  if (/\bwhatsapp (call|meeting)\b/.test(normalized)) {
    return MEETING_PLATFORMS.whatsapp;
  }
  if (/\b(kakaotalk|kakao talk) (call|meeting)\b/.test(normalized)) {
    return MEETING_PLATFORMS.kakaotalk;
  }
  if (/\btelegram (call|meeting)\b/.test(normalized)) {
    return MEETING_PLATFORMS.telegram;
  }
  if (/\bsignal (call|meeting)\b/.test(normalized)) {
    return MEETING_PLATFORMS.signal;
  }
  if (/\bline meeting\b/.test(normalized) || normalized === "line") {
    return MEETING_PLATFORMS.line;
  }
  if (/\bmessenger (call|meeting|room)\b/.test(normalized)) {
    return MEETING_PLATFORMS.messenger;
  }

  return null;
}

export function getBrowserMeetingPlatform(
  apps: MicApp[],
  event: NearbyEvent | null,
): MeetingPlatform | null {
  if (!apps.some(isBrowserApp) || !event) {
    return null;
  }

  if (
    apps.some(
      (app) =>
        !isBrowserApp(app) &&
        getMicAppNotificationOverride(app)?.meetingPlatform,
    )
  ) {
    return null;
  }

  for (const field of [
    "meetingLink",
    "location",
    "description",
    "title",
  ] satisfies Array<keyof NearbyEvent>) {
    const value = event[field];
    if (!value) {
      continue;
    }

    const platform = value.startsWith("http")
      ? detectMeetingPlatformFromUrl(value)
      : detectMeetingPlatformFromText(value);
    if (platform) {
      return platform;
    }
  }

  return null;
}

export function getNotificationDisplayApp(
  app: MicApp,
  browserMeetingPlatform: MeetingPlatform | null,
) {
  if (browserMeetingPlatform && isBrowserApp(app)) {
    return { ...app, name: browserMeetingPlatform.displayName };
  }

  return app;
}

export function getNotificationDisplayApps(
  apps: MicApp[],
  browserMeetingPlatform: MeetingPlatform | null,
) {
  return apps.map((app) =>
    getNotificationDisplayApp(app, browserMeetingPlatform),
  );
}

export async function getNotificationIconForDisplayApp(
  app: MicApp,
  browserMeetingPlatform: MeetingPlatform | null,
): Promise<NotificationIcon | null> {
  if (browserMeetingPlatform && isBrowserApp(app)) {
    return getMeetingPlatformIcon(browserMeetingPlatform);
  }

  return getNotificationIconForApp(app);
}

export async function getNotificationIconForDetectedApps(
  apps: MicApp[],
  browserMeetingPlatform: MeetingPlatform | null,
): Promise<NotificationIcon | null> {
  for (const app of apps) {
    const icon = await getNotificationIconForDisplayApp(
      app,
      browserMeetingPlatform,
    );
    if (icon) {
      return icon;
    }
  }

  return null;
}

export function getIgnorableApps(apps: MicApp[]) {
  const seen = new Set<string>();

  return apps.filter((app) => {
    if (!app.id || app.id.startsWith("pid:") || seen.has(app.id)) {
      return false;
    }

    seen.add(app.id);
    return true;
  });
}

export function getIgnoreAppsFooterText(apps: MicApp[]) {
  const firstName = apps[0] ? getNotificationAppName(apps[0]).trim() : "";

  if (apps.length === 1) {
    return firstName ? `Ignore ${firstName}?` : "Ignore this app?";
  }

  if (!firstName) {
    return "Ignore these apps?";
  }

  const secondName = apps[1] ? getNotificationAppName(apps[1]).trim() : "";
  if (apps.length === 2 && secondName) {
    return `Ignore ${firstName} and ${secondName}?`;
  }

  const otherAppCount = apps.length - 1;
  return `Ignore ${firstName} and ${otherAppCount} other app${otherAppCount === 1 ? "" : "s"}?`;
}
