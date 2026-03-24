import { HeadsetIcon } from "lucide-react";
import { useCallback } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";

import { ListenActionButton } from "../listen-action";
import { FloatingButton } from "./shared";

import { useListenButtonState } from "~/session/components/shared";
import {
  type RemoteMeeting,
  useRemoteMeeting,
} from "~/session/hooks/useRemoteMeeting";
import type { Tab } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";

export function ListenButton({
  tab,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const { shouldRender } = useListenButtonState(tab.id);
  const loading = useListener((state) => state.live.loading);
  const remote = useRemoteMeeting(tab.id);

  if (!remote || loading) {
    return <ListenActionButton sessionId={tab.id} />;
  }

  if (!shouldRender) {
    return null;
  }

  return (
    <div className="flex items-center gap-2">
      <RemoteMeetingButton remote={remote} />
      <ListenActionButton sessionId={tab.id} />
    </div>
  );
}

function RemoteMeetingButton({ remote }: { remote: RemoteMeeting }) {
  const handleJoin = useCallback(() => {
    void openerCommands.openUrl(remote.url, null);
  }, [remote.url]);

  const { icon, name } = getMeetingDisplay(remote.type);

  return (
    <FloatingButton
      onClick={handleJoin}
      className="h-10 justify-center gap-2 border-neutral-200 bg-white px-3 text-neutral-800 shadow-[0_4px_14px_rgba(0,0,0,0.1)] hover:bg-neutral-100 lg:px-4"
    >
      <span>Join</span>
      {icon}
      <span>{name}</span>
    </FloatingButton>
  );
}

function getMeetingDisplay(type: RemoteMeeting["type"]) {
  switch (type) {
    case "zoom":
      return {
        name: "Zoom",
        icon: <img src="/assets/zoom.png" alt="" width={20} height={20} />,
      };
    case "google-meet":
      return {
        name: "Meet",
        icon: <img src="/assets/meet.png" alt="" width={20} height={20} />,
      };
    case "webex":
      return {
        name: "Webex",
        icon: <img src="/assets/webex.png" alt="" width={20} height={20} />,
      };
    case "teams":
      return {
        name: "Teams",
        icon: <img src="/assets/teams.png" alt="" width={20} height={20} />,
      };
    default:
      return {
        name: "Meeting",
        icon: <HeadsetIcon size={20} />,
      };
  }
}
