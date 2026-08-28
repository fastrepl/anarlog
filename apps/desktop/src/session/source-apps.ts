import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";

export type SessionSourceApp = {
  app: string;
  name?: string;
  platform?: string;
};

const SOURCE_APP_ALIASES = new Map<string, string>([
  ["us.zoom.xos", "zoom"],
  ["zoom", "zoom"],
  ["com.microsoft.teams", "teams"],
  ["com.microsoft.teams2", "teams"],
  ["ms-teams", "teams"],
  ["msteams", "teams"],
  ["teams", "teams"],
  ["teams-for-linux", "teams"],
  ["com.slack.slack", "slack"],
  ["com.tinyspeck.slackmacgap", "slack"],
  ["slack", "slack"],
  ["slack-desktop", "slack"],
  ["com.hnc.discord", "discord"],
  ["com.discordapp.discord", "discord"],
  ["discord", "discord"],
  ["cisco-systems.spark", "webex"],
  ["com.cisco.webex", "webex"],
  ["com.cisco.webexmeetingsapp", "webex"],
  ["ciscocollabhost", "webex"],
  ["webex", "webex"],
  ["com.google.chrome", "chrome"],
  ["chrome", "chrome"],
  ["google-chrome", "chrome"],
  ["google-chrome-beta", "chrome"],
  ["google-chrome-stable", "chrome"],
  ["com.microsoft.edgemac", "edge"],
  ["microsoft-edge", "edge"],
  ["microsoft-edge-stable", "edge"],
  ["msedge", "edge"],
  ["org.mozilla.firefox", "firefox"],
  ["firefox", "firefox"],
  ["firefox-bin", "firefox"],
  ["org.chromium.chromium", "chromium"],
  ["chromium", "chromium"],
  ["chromium-browser", "chromium"],
  ["com.apple.safari", "safari"],
  ["safari", "safari"],
  ["com.brave.browser", "brave"],
  ["brave", "brave"],
  ["brave-browser", "brave"],
  ["com.operasoftware.opera", "opera"],
  ["opera", "opera"],
  ["com.vivaldi.vivaldi", "vivaldi"],
  ["vivaldi", "vivaldi"],
  ["at.studio.asidebrowser", "aside"],
  ["aside", "aside"],
  ["com.browseros.browseros", "browseros"],
  ["browseros", "browseros"],
  ["ai.perplexity.comet", "comet"],
  ["comet", "comet"],
  ["company.thebrowser.dia", "dia"],
  ["dia", "dia"],
  ["net.imput.helium", "helium"],
  ["helium", "helium"],
  ["com.nousresearch.hermes", "hermes"],
  ["hermes", "hermes"],
  ["app.zen-browser.zen", "zen"],
  ["zen", "zen"],
]);

export function parseSessionSourceApps(value: string): SessionSourceApp[] {
  try {
    const parsed = JSON.parse(value) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.flatMap((entry) => {
      if (typeof entry !== "object" || entry === null) return [];
      const app = (entry as { app?: unknown }).app;
      if (typeof app !== "string" || !app.trim()) return [];
      const name = (entry as { name?: unknown }).name;
      const platform = (entry as { platform?: unknown }).platform;
      return [
        {
          app,
          ...(typeof name === "string" && name.trim() ? { name } : {}),
          ...(typeof platform === "string" && platform.trim()
            ? { platform }
            : {}),
        },
      ];
    });
  } catch {
    return [];
  }
}

export function mergeSessionSourceApps(
  current: SessionSourceApp[],
  updates: SessionSourceApp[],
): SessionSourceApp[] {
  const merged = current.map((entry) => ({ ...entry }));
  const indexByIdentity = new Map(
    merged.map((entry, index) => [sourceAppIdentity(entry.app), index]),
  );

  for (const update of updates) {
    if (!update.app.trim()) continue;
    const identity = sourceAppIdentity(update.app);
    const index = indexByIdentity.get(identity);
    if (index === undefined) {
      indexByIdentity.set(identity, merged.length);
      merged.push(cleanSourceApp(update));
      continue;
    }

    const existing = merged[index]!;
    merged[index] = {
      app: existing.app,
      ...(update.name?.trim()
        ? { name: update.name }
        : existing.name
          ? { name: existing.name }
          : {}),
      ...(update.platform?.trim()
        ? { platform: update.platform }
        : existing.platform
          ? { platform: existing.platform }
          : {}),
    };
  }

  return merged;
}

export function formatSessionSourceAppsContext(
  sourceApps: SessionSourceApp[] | undefined = [],
): string {
  const platforms = [
    ...new Set(
      sourceApps
        .map((source) => source.platform?.trim())
        .filter((platform): platform is string => Boolean(platform)),
    ),
  ];
  return platforms.length > 0
    ? `Meeting platform: ${platforms.join(", ")}`
    : "";
}

export function recordSessionSourceApps(
  sessionId: string,
  updates: SessionSourceApp[],
): Promise<void> {
  if (!sessionId || updates.length === 0) return Promise.resolve();

  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const rows = await liveQueryClient.execute<{ source_apps_json: string }>(
      `
        SELECT source_apps_json
        FROM sessions
        WHERE id = ? AND deleted_at IS NULL
        LIMIT 1
      `,
      [sessionId],
    );
    const existingJson = rows[0]?.source_apps_json;
    if (existingJson === undefined) return;

    const merged = mergeSessionSourceApps(
      parseSessionSourceApps(existingJson),
      updates,
    );
    const nextJson = JSON.stringify(merged);
    if (nextJson === existingJson) return;

    await executeTransaction([
      {
        sql: `
          UPDATE sessions
          SET source_apps_json = ?, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [nextJson, new Date().toISOString(), sessionId],
      },
    ]);
  });
}

function sourceAppIdentity(app: string): string {
  const pathParts = app.split(/[\\/]/);
  const normalized = (pathParts[pathParts.length - 1] ?? "")
    .trim()
    .toLowerCase()
    .replace(/\.exe$/, "");
  return SOURCE_APP_ALIASES.get(normalized) ?? normalized;
}

function cleanSourceApp(source: SessionSourceApp): SessionSourceApp {
  return {
    app: source.app,
    ...(source.name?.trim() ? { name: source.name } : {}),
    ...(source.platform?.trim() ? { platform: source.platform } : {}),
  };
}
