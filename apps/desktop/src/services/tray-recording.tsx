import { useStore } from "zustand";

import { commands as trayCommands } from "@anlg/plugin-tray";

import { useSession } from "~/session/queries";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { type LiveSessionStatus } from "~/store/zustand/listener/general-shared";
import { listenerStore } from "~/store/zustand/listener/instance";
import {
  resolveLiveSessionTitle,
  useLiveTitle,
} from "~/store/zustand/live-title";

const UNTITLED_SESSION_TITLES = new Set([
  "untitled",
  "untitled event",
  "untitled meeting",
  "untitled note",
]);

const publishTrayRecordingTitle = createTrayRecordingTitlePublisher(
  async (title) => {
    const result = await trayCommands.setTrayRecordingTitle(title);
    if (result.status === "error") {
      console.error("[tray] failed to publish recording title", result.error);
    }
  },
);

export function TrayRecordingSync() {
  const activeSessionId = useStore(listenerStore, (state) =>
    getTrayRecordingSessionId(state.live.status, state.live.sessionId),
  );
  const session = useSession(activeSessionId);
  const liveTitle = useLiveTitle((state) => state.titles[activeSessionId]);
  const title = getTrayRecordingTitle(
    resolveLiveSessionTitle(liveTitle, session?.title),
  );

  return (
    <TrayRecordingPublisher
      key={`${activeSessionId}:${title ?? ""}`}
      title={title}
    />
  );
}

export function getTrayRecordingSessionId(
  status: LiveSessionStatus,
  sessionId: string | null,
): string {
  return status === "active" || status === "finalizing"
    ? (sessionId ?? "")
    : "";
}

export function getTrayRecordingTitle(
  title: string | null | undefined,
): string | null {
  const normalized = title?.trim();
  if (!normalized || UNTITLED_SESSION_TITLES.has(normalized.toLowerCase())) {
    return null;
  }

  return normalized;
}

export function createTrayRecordingTitlePublisher(
  publish: (title: string | null) => Promise<void>,
) {
  let queue = Promise.resolve();

  return (title: string | null) => {
    const publication = queue.then(() => publish(title));
    queue = publication.catch(() => undefined);
    return publication;
  };
}

function TrayRecordingPublisher({ title }: { title: string | null }) {
  useMountEffect(() => {
    void publishTrayRecordingTitle(title).catch((error) => {
      console.error("[tray] failed to publish recording title", error);
    });
  });

  return null;
}
