import type { MeetingPlatform } from "@anlg/plugin-detect";

import {
  recordSessionSourceApps,
  type SessionSourceApp,
} from "~/session/source-apps";
import {
  getMeetingPlatformNameForMicApp,
  type MicApp,
} from "~/stt/meeting-apps";

const CAPTURED_PLATFORM_NAMES: Record<MeetingPlatform, string | null> = {
  zoom: "Zoom",
  googleMeet: "Google Meet",
  microsoftTeams: "Microsoft Teams",
  slack: "Slack",
  discord: "Discord",
  webex: "Webex",
  unknown: null,
};

export function recordDetectedMeetingApps(
  sessionId: string,
  apps: MicApp[],
): Promise<void> {
  return recordSessionSourceApps(
    sessionId,
    apps.map(
      (app): SessionSourceApp => ({
        app: app.id,
        name: app.name,
        platform: getMeetingPlatformNameForMicApp(app) ?? undefined,
      }),
    ),
  );
}

export function recordCapturedMeetingPlatform(
  sessionId: string,
  app: MicApp,
  platform: MeetingPlatform,
): Promise<void> {
  const platformName = CAPTURED_PLATFORM_NAMES[platform];
  if (!platformName) return Promise.resolve();
  return recordSessionSourceApps(sessionId, [
    { app: app.id, name: app.name, platform: platformName },
  ]);
}
