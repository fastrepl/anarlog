import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import { commands as detectCommands } from "@anlg/plugin-detect";
import {
  commands as transcriptionCommands,
  type SpeakerContextInterval,
} from "@anlg/plugin-transcription";

import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { inspectionShowsActiveMeeting } from "~/stt/meeting-accessibility";
import { getSessionParticipantHumanIds } from "~/stt/queries";
import {
  appendSpeakerObservation,
  closeSpeakerContext,
  isSharedMicrophone,
  isPersonalMicrophone,
  parseSpeakerContext,
} from "~/stt/speaker-context";

const captures = new Map<string, ReturnType<typeof createCapture>>();
const POLL_MS = 5_000;
const EVIDENCE_LEASE_MS = 10_000;
// Apps a scheduled meeting link can be joined from; browsers cover Meet and web clients.
const MEETING_CAPABLE_MIC_APP =
  /zoom|teams|slack|webex|chrome|chromium|safari|thebrowser|firefox|brave|edge|vivaldi|opera/i;

export function startSpeakerContextCapture(sessionId: string) {
  if (getCurrentWebviewWindow().label !== "main") return;
  if (!captures.has(sessionId))
    captures.set(sessionId, createCapture(sessionId));
}

export function observeSpeakerMicrophone(
  sessionId: string,
  update: { device?: string | null; isolated?: boolean },
) {
  startSpeakerContextCapture(sessionId);
  captures.get(sessionId)?.microphone(update);
}

export function stopSpeakerContextCapture(sessionId: string) {
  const capture = captures.get(sessionId);
  captures.delete(sessionId);
  return capture?.stop() ?? Promise.resolve();
}

function createCapture(sessionId: string) {
  let stopped = false;
  let device: string | null = null;
  let isolated: boolean | null = null;
  // The app last seen in an active call. Accessibility reads flicker while the call
  // continues, so the call is only over once that app releases the microphone or an
  // inspection positively shows it out of the call.
  let callAppId: string | null = null;
  let generation = 0;
  let pending: Promise<void> | null = null;
  let repoll = false;
  const report = (error: unknown) =>
    console.error("[speaker-context] observation failed", error);
  const persist = (
    observation: SpeakerContextInterval | null,
    at: number,
    observedGeneration?: number,
  ) =>
    enqueueDatabaseWrite(`session:${sessionId}`, async () => {
      const rows = await liveQueryClient.execute<{ context: string | null }>(
        "SELECT json_extract(metadata_json, '$.speaker_context') AS context FROM sessions WHERE id = ? AND deleted_at IS NULL",
        [sessionId],
      );
      if (
        !rows[0] ||
        (observation && (stopped || observedGeneration !== generation))
      )
        return;
      const previous = parseSpeakerContext(rows[0].context);
      const next = observation
        ? appendSpeakerObservation(previous, observation)
        : closeSpeakerContext(previous, at);
      await executeTransaction([
        {
          sql: "UPDATE sessions SET metadata_json = json_set(metadata_json, '$.speaker_context', json(?)), updated_at = ? WHERE id = ? AND deleted_at IS NULL",
          params: [JSON.stringify(next), new Date().toISOString(), sessionId],
        },
      ]);
    });

  const poll = async () => {
    const observedGeneration = generation;
    const at = Date.now();
    const [micResult, inspectionsResult, rows, participantIds, currentDevice] =
      await Promise.all([
        detectCommands.listMicUsingApplications().catch(() => null),
        detectCommands.inspectMeetingAccessibility().catch(() => null),
        liveQueryClient.execute<{
          title: string;
          event_json: string;
          owner_user_id: string;
          name: string;
          aliases: string | null;
        }>(
          "SELECT s.title, s.event_json, s.owner_user_id, COALESCE(h.name, '') AS name, json_extract(h.metadata_json, '$.aliases') AS aliases FROM sessions s LEFT JOIN humans h ON h.id = s.owner_user_id AND h.deleted_at IS NULL WHERE s.id = ? AND s.deleted_at IS NULL",
          [sessionId],
        ),
        getSessionParticipantHumanIds(sessionId),
        device === null
          ? transcriptionCommands.getCurrentMicrophoneDevice().catch(() => null)
          : Promise.resolve(null),
      ]);
    if (stopped || observedGeneration !== generation || !rows[0]) return;
    const row = rows[0];
    const micApps = micResult?.status === "ok" ? micResult.data : [];
    const inspections =
      inspectionsResult?.status === "ok" ? inspectionsResult.data : [];
    const active = inspections.filter(
      (inspection) =>
        inspection.activeCall &&
        inspectionShowsActiveMeeting(inspection) &&
        micApps.some((app) => app.id === inspection.app.id),
    );
    let meetingLink = "";
    try {
      meetingLink = JSON.parse(row.event_json).meeting_link ?? "";
    } catch {
      /* No calendar context. */
    }
    let aliases: string[] = [];
    try {
      const value = JSON.parse(row.aliases ?? "[]");
      if (Array.isArray(value))
        aliases = value.filter((name) => typeof name === "string");
    } catch {
      /* Invalid aliases provide no identity evidence. */
    }
    const participants = participantIds.length
      ? await liveQueryClient.execute<{ human_id: string; name: string }>(
          `SELECT id AS human_id, name FROM humans WHERE id IN (${participantIds.map(() => "?").join(",")}) AND deleted_at IS NULL ORDER BY id`,
          participantIds,
        )
      : [];
    if (stopped || observedGeneration !== generation) return;
    if (active.length === 1) {
      callAppId = active[0]!.app.id;
    } else if (callAppId) {
      // A failed lookup proves nothing; only a successful one can show the app let go.
      const releasedMicrophone =
        micResult?.status === "ok" &&
        !micApps.some((app) => app.id === callAppId);
      const leftCall = inspections.some(
        (inspection) =>
          inspection.app.id === callAppId && !inspection.activeCall,
      );
      if (releasedMicrophone || leftCall) callAppId = null;
    }
    // An observed pre-join/ended window overrides a scheduled link. A link alone is never attendance.
    const calendarCall = Boolean(
      meetingLink &&
      micApps.some((app) => MEETING_CAPABLE_MIC_APP.test(app.id)) &&
      inspections.length === 0,
    );
    const inputDevice =
      device ?? (currentDevice?.status === "ok" ? currentDevice.data : null);
    await persist(
      {
        start_ms: at,
        end_ms: at + EVIDENCE_LEASE_MS,
        active_call: callAppId !== null,
        calendar_call: calendarCall,
        // A headset is isolated by construction; the runtime verdict only tells a room
        // microphone apart from one that also hears the speakers.
        mic_isolated: isPersonalMicrophone(inputDevice)
          ? true
          : isolated === null
            ? null
            : false,
        shared_microphone: isSharedMicrophone(inputDevice),
        title: row.title,
        self_names: [row.name, ...aliases].filter(Boolean),
        participants,
      },
      at,
      observedGeneration,
    );
  };
  const schedule = () => {
    if (stopped || pending) return;
    pending = poll()
      .catch(report)
      .finally(() => {
        pending = null;
        if (repoll) {
          repoll = false;
          schedule();
        }
      });
  };
  const timer = setInterval(schedule, POLL_MS);
  schedule();
  return {
    microphone(update: { device?: string | null; isolated?: boolean }) {
      if ("device" in update) {
        device = update.device ?? null;
        isolated = null;
      }
      if ("isolated" in update) isolated = update.isolated ?? null;
      generation++;
      // Close the old interval immediately; an asynchronous inspection cannot extend old device evidence.
      const at = Date.now();
      void persist(null, at).catch(report);
      repoll = pending !== null;
      schedule();
    },
    stop() {
      stopped = true;
      generation++;
      clearInterval(timer);
      const at = Date.now();
      return persist(null, at).catch(report);
    },
  };
}
