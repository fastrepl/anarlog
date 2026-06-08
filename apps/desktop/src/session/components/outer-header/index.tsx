import { ChevronDownIcon, HeadsetIcon, MicOff } from "lucide-react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { cn, safeParseDate } from "@hypr/utils";

import { MetadataButton } from "./metadata";
import { OverflowButton } from "./overflow";

import { useNow } from "~/calendar/hooks";
import { useShell } from "~/contexts/shell";
import {
  getRemoteMeeting,
  type RemoteMeeting,
} from "~/session/hooks/useRemoteMeeting";
import { useConfigValue } from "~/shared/config";
import { useSessionEvent } from "~/store/tinybase/hooks";
import type { EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";

export function OuterHeader({
  sessionId,
  currentView,
  title,
}: {
  sessionId: string;
  currentView: EditorView;
  title?: React.ReactNode;
}) {
  const { leftsidebar } = useShell();
  const sidebarTimelineEnabled = useConfigValue("sidebar_timeline_enabled");
  const showSidebarTimelineHeaderGutter =
    sidebarTimelineEnabled && !leftsidebar.expanded;

  return (
    <div
      className={cn([
        "flex h-12 w-full items-center",
        showSidebarTimelineHeaderGutter && "pl-[156px]",
      ])}
    >
      <div className="flex w-full min-w-0 items-center justify-between gap-0">
        <div className="flex min-w-0 flex-1 items-center gap-1">
          {title ? <div className="min-w-0 flex-1">{title}</div> : null}
        </div>
        <div className="flex shrink-0 items-center gap-0 pr-1">
          <SidebarModeStopButton sessionId={sessionId} />
          <HeaderMeetingControl sessionId={sessionId} />
          <OverflowButton sessionId={sessionId} currentView={currentView} />
        </div>
      </div>
    </div>
  );
}

function HeaderMeetingControl({ sessionId }: { sessionId: string }) {
  const sessionEvent = useSessionEvent(sessionId);

  if (!sessionEvent) {
    return <MetadataButton sessionId={sessionId} />;
  }

  return <EventMeetingControl sessionId={sessionId} event={sessionEvent} />;
}

function EventMeetingControl({
  sessionId,
  event,
}: {
  sessionId: string;
  event: {
    ended_at?: string;
    meeting_link?: string;
  };
}) {
  const mode = useListener((state) => state.getSessionMode(sessionId));
  const now = useNow();
  const remote = getRemoteMeeting(event.meeting_link);
  const inProgress =
    mode === "active" || mode === "finalizing" || mode === "running_batch";
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
  const { icon, name } = getMeetingDisplay(remote.type);
  const label = `Join ${name}`;
  const handleJoin = () => {
    void openerCommands.openUrl(remote.url, null);
  };

  return (
    <div className="border-border bg-card text-foreground mr-1 flex h-8 max-w-56 shrink-0 items-center overflow-hidden rounded-full border shadow-[0_1px_4px_rgba(0,0,0,0.08)]">
      <button
        type="button"
        aria-label={label}
        title={label}
        onClick={handleJoin}
        className={cn([
          "flex h-full min-w-0 items-center gap-1.5 px-3",
          "text-sm font-medium",
          "hover:bg-accent transition-colors",
        ])}
      >
        {icon}
        <span className="truncate">{label}</span>
      </button>
      <MetadataButton
        sessionId={sessionId}
        renderTrigger={({ open, label: metadataLabel }) => (
          <button
            type="button"
            aria-label={metadataLabel}
            title={metadataLabel}
            className={cn([
              "border-border text-muted-foreground flex h-full w-7 shrink-0 items-center justify-center border-l",
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
    default:
      return {
        name: "Meeting",
        icon: <HeadsetIcon size={18} />,
      };
  }
}

function SidebarModeStopButton({ sessionId }: { sessionId: string }) {
  const { leftsidebar } = useShell();
  const sidebarTimelineEnabled = useConfigValue("sidebar_timeline_enabled");
  const { amplitude, degraded, mode, muted, stop } = useListener((state) => ({
    amplitude: state.live.amplitude,
    degraded: state.live.degraded,
    mode: state.getSessionMode(sessionId),
    muted: state.live.muted,
    stop: state.stop,
  }));
  const active = mode === "active" || mode === "finalizing";
  const finalizing = mode === "finalizing";

  if (!sidebarTimelineEnabled || leftsidebar.expanded || !active) {
    return null;
  }

  const accent = degraded ? "amber" : "red";
  const colors = {
    red: {
      button: "text-red-500 hover:text-red-600 bg-red-50 hover:bg-red-100",
      sticks: "#ef4444",
      stop: "bg-red-500",
    },
    amber: {
      button:
        "text-amber-500 hover:text-amber-600 bg-amber-50 hover:bg-amber-100",
      sticks: "#f59e0b",
      stop: "bg-amber-500",
    },
  }[accent];

  return (
    <button
      type="button"
      onClick={finalizing ? undefined : stop}
      disabled={finalizing}
      className={cn([
        "group inline-flex items-center justify-center rounded-full text-sm font-medium",
        finalizing
          ? ["bg-muted text-muted-foreground cursor-wait"]
          : [colors.button],
        "h-7 w-20",
        "disabled:pointer-events-none disabled:opacity-50",
      ])}
      aria-label={finalizing ? "Finalizing" : "Stop listening"}
    >
      {finalizing ? (
        <div className="flex items-center gap-1.5">
          <span className="animate-pulse">...</span>
        </div>
      ) : (
        <>
          <div
            className={cn(["flex items-center gap-1.5", "group-hover:hidden"])}
          >
            {muted && <MicOff size={14} />}
            <DancingSticks
              amplitude={Math.min(
                Math.hypot(amplitude.mic, amplitude.speaker),
                1,
              )}
              color={colors.sticks}
              height={18}
              width={60}
            />
          </div>
          <div
            className={cn(["hidden items-center gap-1.5", "group-hover:flex"])}
          >
            <span className={cn(["size-2 rounded-none", colors.stop])} />
            <span className="text-xs">Stop</span>
          </div>
        </>
      )}
    </button>
  );
}
