import { Trans, useLingui } from "@lingui/react/macro";
import { ChevronDownIcon, HeadsetIcon, VideoIcon } from "lucide-react";

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

  if (!sessionEvent) {
    return <MetadataButton sessionId={sessionId} />;
  }

  return (
    <EventMeetingControl
      sessionId={sessionId}
      event={sessionEvent}
      sessionMode={sessionMode}
    />
  );
}

function EventMeetingControl({
  sessionId,
  event,
  sessionMode,
}: {
  sessionId: string;
  event: {
    ended_at?: string;
    meeting_link?: string;
  };
  sessionMode: string;
}) {
  const now = useNow();
  const remote = getRemoteMeeting(event.meeting_link);
  const inProgress =
    sessionMode === "active" ||
    sessionMode === "finalizing" ||
    sessionMode === "running_batch";
  const endedAt = event.ended_at ? safeParseDate(event.ended_at) : null;
  const ended = !!endedAt && endedAt.getTime() <= now.getTime();

  if (inProgress) {
    return <MetadataButton sessionId={sessionId} />;
  }

  if (remote && !ended) {
    return <HeaderMeetingJoinButton sessionId={sessionId} remote={remote} />;
  }

  return <MetadataButton sessionId={sessionId} />;
}

function HeaderMeetingJoinButton({
  sessionId,
  remote,
}: {
  sessionId: string;
  remote: RemoteMeeting;
}) {
  const { t } = useLingui();
  const { icon, name } = getMeetingDisplay(remote.type);
  const label = t`Join ${name}`;
  const handleJoin = () => {
    void openerCommands.openUrl(remote.url, null);
  };

  return (
    <div className="border-border bg-card text-foreground mr-1 flex h-7 max-w-56 shrink-0 items-center overflow-hidden rounded-full border">
      <button
        type="button"
        data-tauri-drag-region="false"
        aria-label={label}
        title={label}
        onClick={handleJoin}
        className={cn([
          "flex h-full min-w-0 items-center gap-1.5 px-2.5",
          "text-sm font-medium",
          "hover:bg-accent transition-colors",
        ])}
      >
        <span>
          <Trans>Join</Trans>
        </span>
        {icon}
        <span className="truncate">{name}</span>
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
