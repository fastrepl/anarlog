import { useQuery } from "@tanstack/react-query";

import {
  commands as meetCommands,
  events as meetEvents,
  type MeetClientEvent,
} from "@anlg/plugin-meet-client";

import { applyZoomSpeakerTimeline } from "~/zoom-client";
import {
  applyZoomClientEvent,
  createZoomSpeakerTimeline,
} from "~/zoom-client/speaker-timeline";

export { meetCommands };

// Google Meet has no client SDK; the plugin embeds the Meet web client in a
// webview and reads participants / speaking indicators from its DOM. Dogfood-only.
export function useMeetClientAvailable(): boolean {
  const { data = false } = useQuery({
    queryKey: ["meet-client", "available"],
    queryFn: () => meetCommands.isAvailable(),
    staleTime: Number.POSITIVE_INFINITY,
  });
  return data;
}

// Opens the meeting in the embedded webview and, once it ends, tags transcript
// words with the participants Meet showed as speaking. Returns the plugin session id.
export async function joinMeetInAnarlog({
  sessionId,
  meetingUrl,
}: {
  sessionId: string;
  meetingUrl: string;
}): Promise<string> {
  const timeline = createZoomSpeakerTimeline();
  let meetSessionId: string | null = null;

  const unlisten = await meetEvents.meetClientEvent.listen(
    ({ payload }: { payload: MeetClientEvent }) => {
      if (meetSessionId && payload.data.session_id !== meetSessionId) return;
      applyZoomClientEvent(timeline, payload);
      if (payload.type === "state_changed" && payload.data.state === "ended") {
        unlisten();
        void applyZoomSpeakerTimeline(sessionId, timeline).catch((error) => {
          console.error(
            "[meet-client] failed to apply speaker timeline",
            error,
          );
        });
      }
    },
  );

  const result = await meetCommands.joinMeeting(meetingUrl);
  if (result.status === "error") {
    unlisten();
    throw new Error(result.error);
  }
  meetSessionId = result.data;
  return meetSessionId;
}
