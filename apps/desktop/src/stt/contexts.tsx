import { resolveResource } from "@tauri-apps/api/path";
import React, { createContext, useContext, useEffect, useRef } from "react";
import { useStore } from "zustand";
import { useShallow } from "zustand/shallow";

import {
  commands as detectCommands,
  events as detectEvents,
} from "@hypr/plugin-detect";
import {
  commands as notificationCommands,
  type NotificationIcon,
} from "@hypr/plugin-notification";

import {
  AUTO_STOP_CONFIRM_TIMEOUT_SECONDS,
  createAutoStopEndedNotificationKey,
} from "./auto-stop-notification";

import {
  getNearbyCalendarEvents,
  type NearbyCalendarEvent,
} from "~/calendar/queries";
import { loadSessionEvent } from "~/session/queries";
import { useConfigValue } from "~/shared/config";
import {
  createListenerStore,
  type ListenerStore,
} from "~/store/zustand/listener";

const ListenerContext = createContext<ListenerStore | null>(null);
export const AUTO_STOP_CONFIRM_DELAY_MS = 5_000;
export const AUTO_STOP_CALENDAR_EARLY_END_THRESHOLD_MS = 3 * 60_000;
export const AUTO_STOP_CALENDAR_EARLY_START_BUFFER_MS = 5 * 60_000;

const BROWSER_AUTO_STOP_APP_IDS = new Set([
  "at.studio.AsideBrowser",
  "app.zen-browser.zen",
  "com.apple.Safari",
  "com.apple.SafariTechnologyPreview",
  "com.brave.Browser",
  "com.brave.Browser.beta",
  "com.brave.Browser.nightly",
  "com.duckduckgo.macos.browser",
  "com.google.Chrome",
  "com.google.Chrome.canary",
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

const UNRELIABLE_AUTO_STOP_APP_IDS = new Set(["com.kakao.KakaoTalkMac"]);

type MicApp = { id: string; name: string };
type NearbyEvent = NearbyCalendarEvent;
type MeetingPlatform = {
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
    ids: new Set(["us.zoom.xos"]),
    names: new Set(["zoom", "zoom helper", "zoom workplace"]),
    displayName: "Zoom",
    meetingPlatform: MEETING_PLATFORMS.zoom,
    iconResource: "zoom",
  },
  {
    ids: new Set(["com.microsoft.teams", "com.microsoft.teams2"]),
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
    ids: new Set(["com.slack.Slack", "com.tinyspeck.slackmacgap"]),
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
  return MIC_APP_NOTIFICATION_OVERRIDES.find(
    (override) =>
      override.ids.has(app.id) || override.names.has(normalizedName),
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

async function getNotificationIconForApp(
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

function getNotificationAppName(app: MicApp) {
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

function getBrowserMeetingPlatform(
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

function getNotificationDisplayApp(
  app: MicApp,
  browserMeetingPlatform: MeetingPlatform | null,
) {
  if (browserMeetingPlatform && isBrowserApp(app)) {
    return { ...app, name: browserMeetingPlatform.displayName };
  }

  return app;
}

function getNotificationDisplayApps(
  apps: MicApp[],
  browserMeetingPlatform: MeetingPlatform | null,
) {
  return apps.map((app) =>
    getNotificationDisplayApp(app, browserMeetingPlatform),
  );
}

async function getNotificationIconForDisplayApp(
  app: MicApp,
  browserMeetingPlatform: MeetingPlatform | null,
): Promise<NotificationIcon | null> {
  if (browserMeetingPlatform && isBrowserApp(app)) {
    return getMeetingPlatformIcon(browserMeetingPlatform);
  }

  return getNotificationIconForApp(app);
}

async function getNotificationIconForDetectedApps(
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

function getIgnorableApps(apps: MicApp[]) {
  const seen = new Set<string>();

  return apps.filter((app) => {
    if (!app.id || app.id.startsWith("pid:") || seen.has(app.id)) {
      return false;
    }

    seen.add(app.id);
    return true;
  });
}

function getIgnoreAppsFooterText(apps: MicApp[]) {
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

function parseEventTimeMs(value: string | undefined): number | null {
  if (!value) {
    return null;
  }

  const ms = new Date(value).getTime();
  return Number.isNaN(ms) ? null : ms;
}

async function shouldPromptBeforeAutoStopping({
  appIds,
  sessionId,
  nowMs,
}: {
  appIds: string[];
  sessionId: string | null;
  nowMs: number;
}): Promise<boolean> {
  if (!appIds.some((id) => BROWSER_AUTO_STOP_APP_IDS.has(id))) {
    return false;
  }

  if (!sessionId) {
    return false;
  }

  const event = await loadSessionEvent(sessionId);
  if (!event || event.is_all_day) {
    return false;
  }

  const endMs = parseEventTimeMs(event.ended_at);
  if (!endMs) {
    return false;
  }

  const startMs = parseEventTimeMs(event.started_at);
  if (startMs && nowMs < startMs - AUTO_STOP_CALENDAR_EARLY_START_BUFFER_MS) {
    return false;
  }

  return nowMs < endMs - AUTO_STOP_CALENDAR_EARLY_END_THRESHOLD_MS;
}

function getPrimaryStoppedApp(
  stoppedTriggerAppIds: string[],
  stoppedApps: { id: string; name: string }[],
) {
  return (
    stoppedApps.find(
      (app) =>
        stoppedTriggerAppIds.includes(app.id) &&
        BROWSER_AUTO_STOP_APP_IDS.has(app.id),
    ) ??
    stoppedApps.find((app) => stoppedTriggerAppIds.includes(app.id)) ??
    null
  );
}

function getAutoStopCandidateAppIds(
  triggerAppIds: string[] | null | undefined,
  stoppedApps: { id: string }[],
) {
  const trigger = triggerAppIds ?? [];
  const stoppedIds = new Set(stoppedApps.map((app) => app.id));
  const stoppedTriggerAppIds = trigger.filter((id) => stoppedIds.has(id));
  const candidateAppIds =
    stoppedTriggerAppIds.length > 0 ? stoppedTriggerAppIds : trigger;

  return candidateAppIds.filter((id) => !UNRELIABLE_AUTO_STOP_APP_IDS.has(id));
}

function getAutoStopActiveCheckAppIds(
  triggerAppIds: string[] | null | undefined,
  candidateAppIds: string[],
) {
  const unreliableTriggerAppIds =
    triggerAppIds?.filter((id) => UNRELIABLE_AUTO_STOP_APP_IDS.has(id)) ?? [];

  return [...new Set([...candidateAppIds, ...unreliableTriggerAppIds])];
}

async function showMeetingEndedPrompt({
  sessionId,
  stoppedTriggerAppIds,
  stoppedApps,
}: {
  sessionId: string;
  stoppedTriggerAppIds: string[];
  stoppedApps: { id: string; name: string }[];
}) {
  const app = getPrimaryStoppedApp(stoppedTriggerAppIds, stoppedApps);

  await notificationCommands.showNotification({
    key: createAutoStopEndedNotificationKey(sessionId),
    title: "Did your meeting end?",
    message: `Anarlog will stop listening in ${AUTO_STOP_CONFIRM_TIMEOUT_SECONDS} seconds.`,
    timeout: { secs: AUTO_STOP_CONFIRM_TIMEOUT_SECONDS, nanos: 0 },
    source: null,
    start_time: null,
    participants: null,
    event_details: null,
    action_label: "Stop",
    action_variant: "destructive",
    options: null,
    footer: null,
    icon: app ? await getNotificationIconForApp(app) : null,
  });
}

export const ListenerProvider = ({
  children,
  store,
}: {
  children: React.ReactNode;
  store: ListenerStore;
}) => {
  useHandleDetectEvents(store);

  const storeRef = useRef<ListenerStore | null>(null);
  if (!storeRef.current) {
    storeRef.current = store;
  }

  return (
    <ListenerContext.Provider value={storeRef.current}>
      {children}
    </ListenerContext.Provider>
  );
};

export const useListener = <T,>(
  selector: Parameters<
    typeof useStore<ReturnType<typeof createListenerStore>, T>
  >[1],
) => {
  const store = useContext(ListenerContext);

  if (!store) {
    throw new Error("'useListener' must be used within a 'ListenerProvider'");
  }

  return useStore(store, useShallow(selector));
};

function getMicDetectedNotificationTitle(event: NearbyEvent | null): string {
  if (!event) {
    return "Are you in a meeting?";
  }

  if (event.participantNames.length === 1) {
    return `Are you talking to ${event.participantNames[0]} right now?`;
  }

  if (event.participantNames.length === 2) {
    return `Are you talking to ${event.participantNames[0]} and ${event.participantNames[1]} right now?`;
  }

  return `Are you in ${event.title} right now?`;
}

const useHandleDetectEvents = (store: ListenerStore) => {
  const stop = useStore(store, (state) => state.stop);
  const setMuted = useStore(store, (state) => state.setMuted);
  const autoStopMeetings = useConfigValue("auto_stop_meetings");

  const autoStopMeetingsRef = useRef(autoStopMeetings);
  autoStopMeetingsRef.current = autoStopMeetings;
  const pendingAutoStopRef = useRef<{
    timeout: ReturnType<typeof setTimeout>;
    requireMicSnapshot: boolean;
  } | null>(null);
  const pendingMicDetectedPromptRef = useRef(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    const clearPendingAutoStop = () => {
      if (pendingAutoStopRef.current) {
        clearTimeout(pendingAutoStopRef.current.timeout);
        pendingAutoStopRef.current = null;
      }
    };
    const shouldCaptureMicDetectedTriggerApps = () => {
      const live = store.getState().live;
      return (
        live.status === "active" ||
        (live.status === "inactive" && live.loading && !!live.sessionId)
      );
    };
    const captureTriggerAppIds = (appIds: string[]) => {
      if (appIds.length === 0) {
        return;
      }

      const currentTrigger = store.getState().live.triggerAppIds ?? [];
      if (appIds.some((id) => currentTrigger.includes(id))) {
        clearPendingAutoStop();
      }
      store
        .getState()
        .setTriggerAppIds([...new Set([...currentTrigger, ...appIds])]);
    };

    const confirmAutoStop = async (
      candidateAppIds: string[],
      stoppedApps: { id: string; name: string }[],
      requireMicSnapshot = false,
    ) => {
      const live = store.getState().live;
      if (live.status !== "active") {
        return;
      }

      const currentTrigger = live.triggerAppIds;
      if (
        !currentTrigger ||
        !candidateAppIds.some((id) => currentTrigger.includes(id))
      ) {
        return;
      }

      const activeCheckAppIds = getAutoStopActiveCheckAppIds(
        currentTrigger,
        candidateAppIds,
      );
      const hasUnreliableActiveCheckApp = activeCheckAppIds.some(
        (id) => !candidateAppIds.includes(id),
      );
      const result = await detectCommands.listMicUsingApplications();
      if (result.status === "ok") {
        const activeAppIds = new Set(result.data.map((app) => app.id));
        if (activeCheckAppIds.some((id) => activeAppIds.has(id))) {
          return;
        }
      } else if (requireMicSnapshot || hasUnreliableActiveCheckApp) {
        return;
      }

      const sessionId = store.getState().live.sessionId;
      if (
        await shouldPromptBeforeAutoStopping({
          appIds: candidateAppIds,
          sessionId,
          nowMs: Date.now(),
        })
      ) {
        if (sessionId) {
          await showMeetingEndedPrompt({
            sessionId,
            stoppedTriggerAppIds: candidateAppIds,
            stoppedApps,
          });
        }
        return;
      }

      stop();
    };

    detectEvents.detectEvent
      .listen(({ payload }) => {
        if (payload.type === "micDetected") {
          const ignorableApps = getIgnorableApps(payload.apps);
          const appIds = ignorableApps.map((app) => app.id);

          if (shouldCaptureMicDetectedTriggerApps()) {
            captureTriggerAppIds(appIds);
            return;
          }

          if (pendingMicDetectedPromptRef.current) {
            return;
          }
          pendingMicDetectedPromptRef.current = true;

          void (async () => {
            try {
              const nearbyEvents = await getNearbyCalendarEvents(
                Date.now(),
                15 * 60 * 1000,
              );
              const nearbyEvent = nearbyEvents[0] ?? null;
              const browserMeetingPlatform = getBrowserMeetingPlatform(
                payload.apps,
                nearbyEvent,
              );
              const displayApps = getNotificationDisplayApps(
                payload.apps,
                browserMeetingPlatform,
              );
              const displayIgnorableApps = ignorableApps.map((app) =>
                getNotificationDisplayApp(app, browserMeetingPlatform),
              );

              const footerIcon =
                displayIgnorableApps.length > 0
                  ? await getNotificationIconForDisplayApp(
                      displayIgnorableApps[0]!,
                      browserMeetingPlatform,
                    )
                  : null;
              const notificationIcon = await getNotificationIconForDetectedApps(
                payload.apps,
                browserMeetingPlatform,
              );
              const footer =
                displayIgnorableApps.length > 0
                  ? {
                      text: getIgnoreAppsFooterText(displayIgnorableApps),
                      actionLabel: "Yes",
                      icon: footerIcon,
                    }
                  : null;

              if (shouldCaptureMicDetectedTriggerApps()) {
                captureTriggerAppIds(appIds);
                return;
              }

              await notificationCommands.showNotification({
                key: payload.key,
                title: getMicDetectedNotificationTitle(nearbyEvent),
                message: "",
                timeout: { secs: 15, nanos: 0 },
                source: {
                  type: "mic_detected",
                  app_names: displayApps.map((app) =>
                    getNotificationAppName(app),
                  ),
                  app_ids: appIds,
                  event_ids: nearbyEvent ? [nearbyEvent.id] : [],
                },
                start_time: null,
                participants: null,
                event_details: null,
                action_label: "Yes",
                action_variant: null,
                options: null,
                footer,
                icon: notificationIcon,
              });
            } finally {
              pendingMicDetectedPromptRef.current = false;
            }
          })();
        } else if (payload.type === "micStopped") {
          const autoStopEnabled = autoStopMeetingsRef.current !== false;
          if (!autoStopEnabled) {
            return;
          }

          const trigger = store.getState().live.triggerAppIds;
          const stoppedTriggerAppIds =
            trigger?.filter((id) =>
              payload.apps.some((app) => app.id === id),
            ) ?? [];
          const candidateAppIds = getAutoStopCandidateAppIds(
            trigger,
            payload.apps,
          );
          if (candidateAppIds.length > 0) {
            const requireMicSnapshot = stoppedTriggerAppIds.length === 0;
            if (
              pendingAutoStopRef.current &&
              !pendingAutoStopRef.current.requireMicSnapshot &&
              requireMicSnapshot
            ) {
              return;
            }

            clearPendingAutoStop();

            pendingAutoStopRef.current = {
              timeout: setTimeout(() => {
                pendingAutoStopRef.current = null;
                void confirmAutoStop(
                  candidateAppIds,
                  payload.apps,
                  requireMicSnapshot,
                );
              }, AUTO_STOP_CONFIRM_DELAY_MS),
              requireMicSnapshot,
            };
          }
        } else if (payload.type === "sleepStateChanged") {
          if (payload.value) {
            clearPendingAutoStop();
            stop();
          }
        } else if (payload.type === "micMuted") {
          setMuted(payload.value);
        }
      })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((err) => {
        console.error("Failed to setup detect event listener:", err);
      });

    return () => {
      cancelled = true;
      clearPendingAutoStop();
      unlisten?.();
    };
  }, [stop, setMuted, store]);
};
