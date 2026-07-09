import { useLingui } from "@lingui/react/macro";
import {
  ChevronDownIcon,
  HeadsetIcon,
  SquareIcon,
  VideoIcon,
} from "lucide-react";
import { useCallback } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { cn, safeParseDate } from "@hypr/utils";

import { MetadataButton } from "./metadata";
import { OverflowButton } from "./overflow";

import { useNow } from "~/calendar/hooks";
import { useShell } from "~/contexts/shell";
import {
  getRemoteMeeting,
  type RemoteMeeting,
} from "~/session/hooks/useRemoteMeeting";
import { useSessionEvent } from "~/store/tinybase/hooks";
import type { EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";
import { useStartListening } from "~/stt/useStartListening";
import {
  isMainWebviewWindow,
  requestMainListenerControl,
} from "~/stt/window-control";

export function OuterHeader({
  sessionId,
  currentView,
  standaloneWindow = false,
  title,
  centerTitle = false,
}: {
  sessionId: string;
  currentView: EditorView;
  standaloneWindow?: boolean;
  title?: React.ReactNode;
  centerTitle?: boolean;
}) {
  const { leftsidebar } = useShell();
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const showSidebarTimelineHeaderGutter =
    !standaloneWindow && !leftsidebar.expanded;
  const showExpandedSidebarTimelineHeader = leftsidebar.expanded;

  return (
    <div
      data-tauri-drag-region
      className={cn([
        "relative flex w-full items-center",
        "h-12",
        showSidebarTimelineHeaderGutter && "pl-[156px]",
      ])}
    >
      {title ? (
        <div
          data-tauri-drag-region
          className={cn([
            "pointer-events-none absolute inset-y-0 flex items-center",
            centerTitle && "justify-center",
            "right-[70px]",
            standaloneWindow
              ? "left-[76px]"
              : showSidebarTimelineHeaderGutter
                ? "left-[104px]"
                : showExpandedSidebarTimelineHeader
                  ? "left-0"
                  : "left-[114px]",
          ])}
        >
          <div
            data-tauri-drag-region
            className="pointer-events-auto max-w-full min-w-0"
          >
            {title}
          </div>
        </div>
      ) : null}
      <div
        data-tauri-drag-region
        className="relative z-10 ml-auto flex shrink-0 items-center gap-0 pr-1"
      >
        <HeaderMeetingControl sessionId={sessionId} sessionMode={sessionMode} />
        <OverflowButton
          standaloneWindow={standaloneWindow}
          sessionId={sessionId}
          currentView={currentView}
        />
      </div>
    </div>
  );
}

function HeaderMeetingControl({
  sessionId,
  sessionMode,
}: {
  sessionId: string;
  sessionMode: string;
}) {
  const sessionEvent = useSessionEvent(sessionId);
  const now = useNow();

  return (
    <HeaderMeetingActionPill
      sessionId={sessionId}
      event={sessionEvent}
      now={now}
      sessionMode={sessionMode}
    />
  );
}

function HeaderMeetingActionPill({
  sessionId,
  event,
  now,
  sessionMode,
}: {
  sessionId: string;
  event: { ended_at?: string; meeting_link?: string } | null;
  now: Date;
  sessionMode: string;
}) {
  const startListening = useStartListening(sessionId);
  const { stop, stopTranscription } = useListener((state) => ({
    stop: state.stop,
    stopTranscription: state.stopTranscription,
  }));
  const remote = getRemoteMeeting(event?.meeting_link);
  const meetingLink = event?.meeting_link || null;
  const endedAt = event?.ended_at ? safeParseDate(event.ended_at) : null;
  const ended = !!endedAt && endedAt.getTime() <= now.getTime();
  const { t } = useLingui();
  const start = useCallback(() => {
    if (!isMainWebviewWindow()) {
      void requestMainListenerControl("start", sessionId);
      return;
    }

    void startListening();
  }, [sessionId, startListening]);
  const stopListening = useCallback(() => {
    if (!isMainWebviewWindow()) {
      void requestMainListenerControl("stop", sessionId);
      return;
    }

    stop();
  }, [sessionId, stop]);
  const action = (() => {
    if (sessionMode === "active") {
      return {
        label: t`Stop`,
        title: t`Stop listening`,
        icon: <SquareIcon className="size-3 fill-current" />,
        onClick: stopListening,
      };
    }

    if (sessionMode === "running_batch") {
      return {
        label: t`Stop`,
        title: t`Stop transcription`,
        icon: <SquareIcon className="size-3 fill-current" />,
        onClick: () => {
          void stopTranscription(sessionId);
        },
      };
    }

    if (meetingLink && !ended) {
      return {
        label: t`Join & start`,
        title: t`Join meeting and start listening`,
        icon: remote ? getMeetingDisplay(remote.type).icon : undefined,
        onClick: () => {
          void openerCommands.openUrl(meetingLink, null);
          start();
        },
      };
    }

    if (ended) {
      return {
        label: t`Resume`,
        title: t`Resume listening`,
        icon: undefined,
        onClick: start,
      };
    }

    return {
      label: t`Start listening`,
      title: t`Start listening`,
      icon: undefined,
      onClick: start,
    };
  })();
  const disabled = sessionMode === "finalizing";

  return (
    <div className="border-border bg-card text-foreground mr-1 flex h-7 max-w-56 shrink-0 items-center overflow-hidden rounded-full border">
      <button
        type="button"
        data-tauri-drag-region="false"
        aria-label={action.label}
        title={action.title}
        disabled={disabled}
        onClick={action.onClick}
        className={cn([
          "flex h-full min-w-0 items-center gap-1.5 px-2.5",
          "text-sm font-medium",
          "hover:bg-accent transition-colors",
          disabled && "cursor-default opacity-60 hover:bg-transparent",
        ])}
      >
        {action.icon}
        <span className="truncate">{action.label}</span>
      </button>
      <MetadataButton
        sessionId={sessionId}
        renderTrigger={({ open, label: metadataLabel }) => (
          <button
            type="button"
            data-tauri-drag-region="false"
            aria-label={metadataLabel}
            title={metadataLabel}
            className={cn([
              "text-muted-foreground flex h-full w-[26px] shrink-0 items-center justify-start pl-[5px]",
              "hover:bg-accent hover:text-foreground transition-colors",
              open && "bg-accent text-foreground",
            ])}
          >
            <ChevronDownIcon size={14} />
          </button>
        )}
      />
    </div>
  );
}

function getMeetingDisplay(type: RemoteMeeting["type"]) {
  switch (type) {
    case "zoom":
      return {
        name: "Zoom",
        icon: <img src="/assets/zoom.png" alt="" width={18} height={18} />,
      };
    case "google-meet":
      return {
        name: "Meet",
        icon: <img src="/assets/meet.png" alt="" width={18} height={18} />,
      };
    case "webex":
      return {
        name: "Webex",
        icon: <img src="/assets/webex.png" alt="" width={18} height={18} />,
      };
    case "teams":
      return {
        name: "Teams",
        icon: <img src="/assets/teams.png" alt="" width={18} height={18} />,
      };
    case "cal-com":
      return {
        name: "Cal.com",
        icon: <VideoIcon size={18} />,
      };
    default:
      return {
        name: "Meeting",
        icon: <HeadsetIcon size={18} />,
      };
  }
}
