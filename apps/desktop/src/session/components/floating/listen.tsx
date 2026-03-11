import { HeadsetIcon } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Spinner } from "@hypr/ui/components/ui/spinner";

import { ListenActionButton } from "../listen-action";
import { OptionsMenu } from "./options-menu";
import { ActionableTooltipContent, FloatingButton } from "./shared";

import { useShell } from "~/contexts/shell";
import {
  RecordingIcon,
  useListenButtonState,
} from "~/session/components/shared";
import {
  type RemoteMeeting,
  useRemoteMeeting,
} from "~/session/hooks/useRemoteMeeting";
import { useEventCountdown } from "~/sidebar/useEventCountdown";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { useStartListening } from "~/stt/useStartListening";

export function ListenButton({
  tab,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const { shouldRender } = useListenButtonState(tab.id);
  const { loading, stop } = useListener((state) => ({
    loading: state.live.loading,
    stop: state.stop,
  }));
  const remote = useRemoteMeeting(tab.id);

  if (loading) {
    return (
      <FloatingButton onClick={stop}>
        <Spinner />
      </FloatingButton>
    );
  }

  if (!shouldRender) {
    return null;
  }

  if (remote) {
    return <SplitMeetingButtons remote={remote} tab={tab} />;
  }

  return <ListenActionButton sessionId={tab.id} />;
}

const SIDEBAR_WIDTH = 280;
const LAYOUT_PADDING = 4;
const EDITOR_WIDTH_THRESHOLD = 590;

function SplitMeetingButtons({
  remote,
  tab,
}: {
  remote: RemoteMeeting;
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const { isDisabled, warningMessage } = useListenButtonState(tab.id);
  const startListening = useStartListening(tab.id);
  const openNew = useTabs((state) => state.openNew);
  const countdown = useEventCountdown(tab.id);
  const { leftsidebar } = useShell();
  const [isNarrow, setIsNarrow] = useState(false);

  useEffect(() => {
    const calculateIsNarrow = () => {
      const sidebarOffset = leftsidebar.expanded
        ? SIDEBAR_WIDTH + LAYOUT_PADDING
        : 0;
      const availableWidth = window.innerWidth - sidebarOffset;
      setIsNarrow(availableWidth < EDITOR_WIDTH_THRESHOLD);
    };

    calculateIsNarrow();
    window.addEventListener("resize", calculateIsNarrow);
    return () => window.removeEventListener("resize", calculateIsNarrow);
  }, [leftsidebar.expanded]);

  const handleJoin = useCallback(() => {
    if (remote.url) {
      void openerCommands.openUrl(remote.url, null);
    }
  }, [remote.url]);

  const handleConfigure = useCallback(() => {
    startListening();
    openNew({ type: "ai", state: { tab: "transcription" } });
  }, [startListening, openNew]);

  const getMeetingIcon = () => {
    switch (remote.type) {
      case "zoom":
        return <img src="/assets/zoom.png" width={20} height={20} />;
      case "google-meet":
        return <img src="/assets/meet.png" width={20} height={20} />;
      case "webex":
        return <img src="/assets/webex.png" width={20} height={20} />;
      case "teams":
        return <img src="/assets/teams.png" width={20} height={20} />;
      default:
        return <HeadsetIcon size={20} />;
    }
  };

  const getMeetingName = () => {
    switch (remote.type) {
      case "zoom":
        return "Zoom";
      case "google-meet":
        return "Meet";
      case "webex":
        return "Webex";
      case "teams":
        return "Teams";
    }
  };

  return (
    <div className="relative flex items-center gap-2">
      {!isNarrow && (
        <FloatingButton
          onClick={handleJoin}
          className="h-10 justify-center gap-2 border-neutral-200 bg-white px-3 text-neutral-800 shadow-[0_4px_14px_rgba(0,0,0,0.1)] hover:bg-neutral-100 lg:px-4"
        >
          <span>Join</span>
          {getMeetingIcon()}
          <span>{getMeetingName()}</span>
        </FloatingButton>
      )}
      <OptionsMenu
        sessionId={tab.id}
        disabled={isDisabled}
        warningMessage={warningMessage}
        onConfigure={handleConfigure}
      >
        <FloatingButton
          onClick={startListening}
          disabled={isDisabled}
          className="justify-center gap-2 border-stone-600 bg-stone-800 pr-8 pl-3 text-white shadow-[0_4px_14px_rgba(87,83,78,0.4)] hover:bg-stone-700 lg:pr-10 lg:pl-4"
          tooltip={
            warningMessage
              ? {
                  side: "top",
                  content: (
                    <ActionableTooltipContent
                      message={warningMessage}
                      action={{
                        label: "Configure",
                        handleClick: handleConfigure,
                      }}
                    />
                  ),
                }
              : undefined
          }
        >
          <span className="flex items-center gap-2 pl-3">
            <RecordingIcon /> Start listening
          </span>
        </FloatingButton>
      </OptionsMenu>
      {countdown && (
        <div className="absolute bottom-full left-1/2 mb-2 -translate-x-1/2 text-xs whitespace-nowrap text-neutral-500">
          {countdown}
        </div>
      )}
    </div>
  );
}
