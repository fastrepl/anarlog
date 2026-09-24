import { useQuery } from "@tanstack/react-query";

import {
  commands as zoomCommands,
  events as zoomEvents,
  type ZoomClientEvent,
  type ZoomParticipant,
} from "@anlg/plugin-zoom-client";

import {
  applyZoomClientEvent,
  createZoomSpeakerTimeline,
  wordIdsByParticipant,
  type ZoomSpeakerTimeline,
} from "./speaker-timeline";

import { liveQueryClient, useLiveQuery } from "~/db";
import { useOwnerUserId } from "~/shared/owner-user";
import {
  assignTranscriptWordsToHuman,
  getSessionTranscriptWords,
} from "~/stt/queries";

export { zoomCommands };

// The embedded Zoom client is dogfood-only until the Zoom Marketplace review
// clears; the plugin reports `false` in release builds without the opt-in.
export function useZoomClientAvailable(): boolean {
  const { data = false } = useQuery({
    queryKey: ["zoom-client", "available"],
    queryFn: () => zoomCommands.isAvailable(),
    staleTime: Number.POSITIVE_INFINITY,
  });
  return data;
}

export function useOwnerDisplayName(): string {
  const ownerUserId = useOwnerUserId();
  const { data } = useLiveQuery<{ name: string }, string>({
    sql: `SELECT name FROM humans WHERE id = ? AND deleted_at IS NULL`,
    params: [ownerUserId ?? ""],
    enabled: Boolean(ownerUserId),
    mapRows: (rows) => rows[0]?.name.trim() ?? "",
  });
  return data || "Anarlog";
}

type SessionHumanRow = { id: string; name: string; email: string };

async function humanIdsForParticipants(
  sessionId: string,
  participants: Iterable<ZoomParticipant>,
): Promise<Map<string, string>> {
  const rows = await liveQueryClient.execute<SessionHumanRow>(
    `
      SELECT human.id, human.name, human.email
      FROM session_participants AS participant
      JOIN humans AS human
        ON human.id = participant.human_id AND human.deleted_at IS NULL
      WHERE participant.session_id = ?
        AND participant.source <> 'excluded'
        AND participant.deleted_at IS NULL
    `,
    [sessionId],
  );
  const byEmail = new Map<string, string>();
  const byName = new Map<string, string>();
  for (const row of rows) {
    if (row.email) byEmail.set(row.email.trim().toLowerCase(), row.id);
    if (row.name) byName.set(row.name.trim().toLowerCase(), row.id);
  }

  const result = new Map<string, string>();
  for (const participant of participants) {
    const email = participant.email?.trim().toLowerCase();
    const name = participant.display_name?.trim().toLowerCase();
    const humanId =
      (email && byEmail.get(email)) || (name && byName.get(name)) || null;
    if (humanId) result.set(participant.id, humanId);
  }
  return result;
}

export async function applyZoomSpeakerTimeline(
  sessionId: string,
  timeline: ZoomSpeakerTimeline,
): Promise<void> {
  const humans = await humanIdsForParticipants(
    sessionId,
    timeline.participants.values(),
  );
  if (humans.size === 0) return;

  const transcripts = await getSessionTranscriptWords(sessionId);
  for (const transcript of transcripts) {
    const grouped = wordIdsByParticipant(
      transcript.words,
      transcript.started_at_ms,
      timeline,
    );
    const assignments = [...grouped].flatMap(([participantId, wordIds]) => {
      const humanId = humans.get(participantId);
      return humanId ? [{ humanId, wordIds }] : [];
    });
    if (assignments.length > 0) {
      await assignTranscriptWordsToHuman({
        transcriptId: transcript.id,
        assignments,
      });
    }
  }
}

// Joins the meeting in-app and, once the meeting ends, tags transcript words with
// the participants Zoom reported as speaking. Returns the plugin session id.
export async function joinMeetingInAnarlog({
  sessionId,
  meetingUrl,
  displayName,
}: {
  sessionId: string;
  meetingUrl: string;
  displayName: string;
}): Promise<string> {
  const timeline = createZoomSpeakerTimeline();
  let zoomSessionId: string | null = null;

  const unlisten = await zoomEvents.zoomClientEvent.listen(
    ({ payload }: { payload: ZoomClientEvent }) => {
      if (zoomSessionId && payload.data.session_id !== zoomSessionId) return;
      applyZoomClientEvent(timeline, payload);
      if (payload.type === "state_changed" && payload.data.state === "ended") {
        unlisten();
        void applyZoomSpeakerTimeline(sessionId, timeline).catch((error) => {
          console.error(
            "[zoom-client] failed to apply speaker timeline",
            error,
          );
        });
      }
    },
  );

  const result = await zoomCommands.joinMeeting(meetingUrl, displayName);
  if (result.status === "error") {
    unlisten();
    throw new Error(result.error);
  }
  zoomSessionId = result.data;
  return zoomSessionId;
}
