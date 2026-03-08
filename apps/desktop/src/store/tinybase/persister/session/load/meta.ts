import type { LoadedSessionData } from "./types";

import type { SessionMetaJson } from "~/store/tinybase/persister/session/types";

const LABEL = "SessionPersister";

export function extractSessionIdAndFolder(path: string): {
  sessionId: string;
  folderPath: string;
} {
  const parts = path.split("/");
  const sessionId = parts[parts.length - 2] || "";
  let folderPath = parts.slice(0, -2).join("/");

  if (folderPath === "sessions") {
    folderPath = "";
  } else if (folderPath.startsWith("sessions/")) {
    folderPath = folderPath.slice("sessions/".length);
  }

  return { sessionId, folderPath };
}

export function processMetaFile(
  path: string,
  content: string,
  result: LoadedSessionData,
): void {
  const { sessionId, folderPath } = extractSessionIdAndFolder(path);
  if (!sessionId) return;

  try {
    const meta = JSON.parse(content) as SessionMetaJson;

    const eventValue = meta.event ? JSON.stringify(meta.event) : undefined;

    result.sessions[sessionId] = {
      user_id: meta.user_id ?? "",
      created_at: meta.created_at ?? "",
      title: meta.title ?? "",
      folder_id: folderPath,
      event_json: eventValue,
      raw_md: "",
    };

    for (const participant of meta.participants) {
      result.mapping_session_participant[participant.id] = {
        user_id: participant.user_id,
        session_id: sessionId,
        human_id: participant.human_id,
        source: participant.source,
      };
    }

    if (meta.tags) {
      for (const tagName of meta.tags) {
        if (!result.tags[tagName]) {
          result.tags[tagName] = {
            user_id: meta.user_id,
            name: tagName,
          };
        }

        const mappingId = `${sessionId}:${tagName}`;
        result.mapping_tag_session[mappingId] = {
          user_id: meta.user_id,
          tag_id: tagName,
          session_id: sessionId,
        };
      }
    }
  } catch (error) {
    console.error(`[${LABEL}] Failed to parse meta from ${path}:`, error);
  }
}
