import { emit, listen } from "@tauri-apps/api/event";
import { useCallback, useState } from "react";

import { commands as listenerCommands } from "@anlg/plugin-transcription";
import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

import { useStartListeningWithBatchOverride } from "./useStartListeningWithBatchOverride";

import { createSession } from "~/session/queries";
import { useConfigValue } from "~/shared/config";
import { useLatestRef } from "~/shared/hooks/useLatestRef";
import { folderIdForNewNote, useSidebarNotes } from "~/sidebar/note-filter";
import { listenerStore } from "~/store/zustand/listener/instance";
import { useTabs } from "~/store/zustand/tabs";

// The Meet extension's start, from crates/transcription-core/src/browser_meeting/start.rs: the app
// asks for a note, reserves the capture of this request for the extension, then says go; this
// window records it with the record button's own path and the person's selected providers,
// carrying the request id so only this start's capture becomes the extension's.
export const START_REQUEST_EVENT = "anlg:browser-meeting-start";
export const START_GO_EVENT = "anlg:browser-meeting-start-go";
export const START_REPLY_EVENT = "anlg:browser-meeting-start-reply";
const GO_TIMEOUT_MS = 10_000;

const ALREADY_RECORDING =
  "Anarlog is already recording a meeting. Stop it first.";
const NEEDS_CALL_RECORDER =
  "Only Anarlog's call recorder can confirm a Meet call, and it isn't on for this microphone. Start the recording in Anarlog instead.";
const COULDNT_CREATE_NOTE = "Anarlog couldn't create a note for this meeting.";

type StartReply =
  | { status: "session"; requestId: string; sessionId: string }
  | { status: "refused"; requestId: string; message: string }
  | {
      status: "done";
      requestId: string;
      sessionId: string;
      message: string | null;
    };

function reply(payload: StartReply) {
  return emit(START_REPLY_EVENT, payload).catch((error) => {
    console.error("[browser-meeting] failed to answer a start", error);
  });
}

// The same note the record shortcut creates: in the folder the sidebar shows.
function createNoteSession() {
  const { noteFilter, folderFilter } = useSidebarNotes.getState();
  const folderId = folderIdForNewNote(noteFilter, folderFilter);
  return folderId === undefined
    ? createSession()
    : createSession("", undefined, { folder_id: folderId });
}

async function listenForGo(requestId: string) {
  let settle!: (go: boolean) => void;
  const answer = new Promise<boolean>((resolve) => {
    settle = resolve;
  });
  const unlisten = await listen<{ requestId: string; go: boolean }>(
    START_GO_EVENT,
    (event) => {
      if (event.payload.requestId === requestId) {
        settle(event.payload.go);
      }
    },
  );
  const timer = setTimeout(() => settle(false), GO_TIMEOUT_MS);
  return {
    go: answer.finally(() => {
      clearTimeout(timer);
      unlisten();
    }),
  };
}

/** Creates and opens the note to record, or says why not. Resolves to it once the app says go. */
async function prepareStart(requestId: string, micDevice: string | null) {
  const refuse = async (message: string) => {
    await reply({ status: "refused", requestId, message });
    return null;
  };
  const { live } = listenerStore.getState();
  if (live.loading || live.status === "active") {
    return refuse(ALREADY_RECORDING);
  }
  // A recording the legacy engine makes never confirms a call, so the tab couldn't follow it.
  let engine: string | null = null;
  try {
    const capability =
      await listenerCommands.getMeetingCaptureCapability(micDevice);
    engine = capability.status === "ok" ? capability.data.engine : null;
  } catch (error) {
    console.error("[browser-meeting] failed to read the capture engine", error);
  }
  if (engine !== "native_meeting") {
    return refuse(NEEDS_CALL_RECORDER);
  }

  let sessionId: string;
  try {
    sessionId = await createNoteSession();
  } catch (error) {
    console.error("[browser-meeting] failed to create a note", error);
    return refuse(COULDNT_CREATE_NOTE);
  }
  useTabs.getState().openNew({ type: "sessions", id: sessionId });

  const { go } = await listenForGo(requestId);
  await reply({ status: "session", requestId, sessionId });
  return (await go) ? sessionId : null;
}

/** Answers the Meet extension's start requests in the main window, one at a time. */
export function BrowserMeetingStartBridge() {
  const [requests, setRequests] = useState<string[]>([]);

  useMountEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void listen<{ requestId: string }>(START_REQUEST_EVENT, (event) => {
      if (active) {
        setRequests((current) => [...current, event.payload.requestId]);
      }
    }).then((fn) => {
      if (active) {
        unlisten = fn;
      } else {
        fn();
      }
    });
    return () => {
      active = false;
      unlisten?.();
    };
  });

  const handled = useCallback((requestId: string) => {
    setRequests((current) => current.filter((id) => id !== requestId));
  }, []);

  const requestId = requests[0];
  if (!requestId) {
    return null;
  }
  return (
    <StartRequestRunner
      key={requestId}
      requestId={requestId}
      onHandled={handled}
    />
  );
}

function StartRequestRunner({
  requestId,
  onHandled,
}: {
  requestId: string;
  onHandled: (requestId: string) => void;
}) {
  const microphoneDevice = useConfigValue("microphone_device");
  const microphoneRef = useLatestRef(microphoneDevice);
  const [sessionId, setSessionId] = useState<string | null>(null);

  // One request per mount (keyed by its id); the app waits on its answers.
  useMountEffect(() => {
    void prepareStart(requestId, microphoneRef.current || null).then(
      (session) => {
        if (session) {
          setSessionId(session);
        } else {
          onHandled(requestId);
        }
      },
    );
  });

  if (!sessionId) {
    return null;
  }
  return (
    <StartListeningRunner
      requestId={requestId}
      sessionId={sessionId}
      onHandled={onHandled}
    />
  );
}

function StartListeningRunner({
  requestId,
  sessionId,
  onHandled,
}: {
  requestId: string;
  sessionId: string;
  onHandled: (requestId: string) => void;
}) {
  const startListening = useStartListeningWithBatchOverride(sessionId, {
    extensionStart: requestId,
  });
  const startRef = useLatestRef(startListening);

  useMountEffect(() => {
    void (async () => {
      try {
        await startRef.current();
      } catch (error) {
        console.error("[browser-meeting] failed to start recording", error);
      }
      // The record path reports its own failures in this window. The app knows from the
      // recorder whether this request's capture began; the tab only hears that it didn't start.
      await reply({ status: "done", requestId, sessionId, message: null });
      onHandled(requestId);
    })();
  });

  return null;
}
