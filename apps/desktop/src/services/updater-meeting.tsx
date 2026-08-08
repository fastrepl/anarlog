import { useStore } from "zustand";

import { commands as updaterCommands } from "@anlg/plugin-updater2";

import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { listenerStore } from "~/store/zustand/listener/instance";

// Serialized so rapid start/stop transitions publish in order.
const publishMeetingActive = (() => {
  let queue = Promise.resolve();
  return (active: boolean) => {
    const publication = queue.then(async () => {
      await updaterCommands.setMeetingActive(active);
    });
    queue = publication.catch(() => undefined);
    return publication;
  };
})();

// Mirrors live-meeting state into the updater2 plugin so the Rust side never
// auto-installs (restarting the app) or auto-downloads during a recording.
export function UpdaterMeetingSync() {
  const meetingActive = useStore(
    listenerStore,
    (state) =>
      state.live.status === "active" || state.live.status === "finalizing",
  );

  return (
    <UpdaterMeetingPublisher
      key={String(meetingActive)}
      active={meetingActive}
    />
  );
}

function UpdaterMeetingPublisher({ active }: { active: boolean }) {
  useMountEffect(() => {
    void publishMeetingActive(active).catch((error) => {
      console.error("[updater] failed to publish meeting state", error);
    });
  });

  return null;
}
