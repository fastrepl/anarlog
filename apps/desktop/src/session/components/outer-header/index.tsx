import { useLingui } from "@lingui/react/macro";
import { CaretDown, Headset, Square, VideoCamera } from "@phosphor-icons/react";
import { useCallback, useRef, useState } from "react";

import { commands as deeplinkCommands } from "@anlg/plugin-deeplink2";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { cn, safeParseDate } from "@anlg/utils";

import { RecordingIcon, useHasTranscript } from "../shared";
import { MetadataButton } from "./metadata";
import { OverflowButton } from "./overflow";

import { useAudioPlayer } from "~/audio-player";
import { useNow } from "~/calendar/hooks";
import { useShell } from "~/contexts/shell";
import {
  buildWelcomeNoteDemoUrl,
  WELCOME_NOTE_TRACKING_ID,
} from "~/onboarding/welcome-note.constants";
import { SessionShareButton } from "~/session-sharing";
import { useEventCountdown } from "~/session/hooks/useEventCountdown";
import {
  getRemoteMeeting,
  type RemoteMeeting,
} from "~/session/hooks/useRemoteMeeting";
import { useSessionEvent } from "~/session/hooks/useSessionEvent";
import { useConfigValue } from "~/shared/config";
import { useWindowControlsGutter } from "~/shared/hooks/useWindowControlsGutter";
import { getScheme } from "~/shared/utils";
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
  const showWindowControlsGutter = useWindowControlsGutter();
  const showSidebarTimelineHeaderGutter =
    !standaloneWindow && !leftsidebar.expanded;
  const showExpandedSidebarTimelineHeader = leftsidebar.expanded;

  return (
    <div
      data-tauri-drag-region
      className={cn([
        "relative flex w-full items-center",
        "h-12",
        showSidebarTimelineHeaderGutter &&
          (showWindowControlsGutter ? "pl-[156px]" : "pl-[80px]"),
      ])}
    >
      {title ? (
        <div
          data-tauri-drag-region
          className={cn([
            "pointer-events-none absolute inset-y-0 flex items-center",
            centerTitle && "justify-center",
            "right-[140px]",
            standaloneWindow
              ? showWindowControlsGutter
                ? "left-[76px]"
                : "left-2"
              : showSidebarTimelineHeaderGutter
                ? showWindowControlsGutter
                  ? "left-[104px]"
                  : "left-[28px]"
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
        <SessionShareButton key={sessionId} sessionId={sessionId} />
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
  const endedAt = sessionEvent?.ended_at
    ? safeParseDate(sessionEvent.ended_at)
    : null;
  const ended = !!endedAt && endedAt.getTime() <= now.getTime();
  const isRecording =
    sessionMode === "active" || sessionMode === "running_batch";

  if (ended && !isRecording) {
    return (
      <div className="mr-1 shrink-0">
        <MetadataButton sessionId={sessionId} />
      </div>
    );
  }

  return (
    <HeaderMeetingActionPill
      sessionId={sessionId}
      event={sessionEvent}
      sessionMode={sessionMode}
    />
  );
}

function HeaderMeetingActionPill({
  sessionId,
  event,
  sessionMode,
}: {
  sessionId: string;
  event: {
    meeting_link?: string;
    tracking_id?: string;
  } | null;
  sessionMode: string;
}) {
  const startListening = useStartListening(sessionId);
  const { canStartLiveSession, stop, stopTranscription } = useListener(
    (state) => ({
      canStartLiveSession: state.canStartLiveSession(sessionId),
      stop: state.stop,
      stopTranscription: state.stopTranscription,
    }),
  );
  const autoJoinScheduledMeetings = useConfigValue(
    "auto_join_scheduled_meetings",
  );
  const autoStartScheduledMeetings = useConfigValue(
    "auto_start_scheduled_meetings",
  );
  const remote = getRemoteMeeting(event?.meeting_link);
  const meetingLink = event?.meeting_link || null;
  const hasTranscript = useHasTranscript(sessionId);
  const { audioExists } = useAudioPlayer();
  const canResume = audioExists || hasTranscript;
  const { t } = useLingui();
  const joiningMeetingRef = useRef(false);
  const [joiningMeeting, setJoiningMeeting] = useState(false);
  const start = useCallback(async () => {
    if (!isMainWebviewWindow()) {
      await requestMainListenerControl("start", sessionId);
      return;
    }

    await startListening();
  }, [sessionId, startListening]);
  const openMeeting = useCallback(async () => {
    if (!meetingLink) {
      return;
    }

    let url = meetingLink;
    if (event?.tracking_id === WELCOME_NOTE_TRACKING_ID) {
      try {
        const scheme = await getScheme();
        const result = await deeplinkCommands.startCallbackServer(scheme);
        if (result.status === "ok") {
          url = buildWelcomeNoteDemoUrl(meetingLink, result.data);
        }
      } catch (error) {
        console.error(
          "[onboarding] failed to prepare demo completion callback",
          error,
        );
      }
    }

    void openerCommands.openUrl(url, null);
  }, [event?.tracking_id, meetingLink]);
  const joinMeeting = useCallback(async () => {
    if (joiningMeetingRef.current) {
      return;
    }

    joiningMeetingRef.current = true;
    setJoiningMeeting(true);
    try {
      await Promise.all([openMeeting(), start()]);
    } finally {
      joiningMeetingRef.current = false;
      setJoiningMeeting(false);
    }
  }, [openMeeting, start]);
  const handleCountdownExpire = useCallback(() => {
    if (!autoStartScheduledMeetings || !canStartLiveSession) {
      return;
    }

    if (autoJoinScheduledMeetings && meetingLink) {
      void joinMeeting();
    } else {
      void start();
    }
  }, [
    autoJoinScheduledMeetings,
    autoStartScheduledMeetings,
    canStartLiveSession,
    joinMeeting,
    meetingLink,
    start,
  ]);
  const countdown = useEventCountdown(sessionId, {
    onExpire: handleCountdownExpire,
  });
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
        icon: <Square className="size-3 text-red-500" weight="fill" />,
        onClick: stopListening,
      };
    }

    if (sessionMode === "running_batch") {
      return {
        label: t`Stop`,
        title: t`Stop transcription`,
        icon: <Square className="size-3 text-red-500" weight="fill" />,
        onClick: () => {
          void stopTranscription(sessionId);
        },
      };
    }

    if (meetingLink) {
      return {
        label: t`Join & record`,
        title: t`Join meeting and record`,
        icon:
          event?.tracking_id === WELCOME_NOTE_TRACKING_ID ? (
            <img src="/assets/anarlog-icon.png" alt="" className="size-4" />
          ) : remote ? (
            getMeetingDisplay(remote.type).icon
          ) : undefined,
        onClick: () => {
          void joinMeeting();
        },
      };
    }

    return {
      label: canResume ? t`Resume` : t`Record`,
      title: canResume ? t`Resume listening` : t`Record`,
      icon: <RecordingIcon />,
      onClick: start,
    };
  })();
  const disabled = sessionMode === "finalizing" || joiningMeeting;
  const showCountdown =
    Boolean(countdown.label) &&
    sessionMode !== "active" &&
    sessionMode !== "running_batch" &&
    sessionMode !== "finalizing";

  return (
    <div className="relative mr-1 flex min-w-0 shrink-0 items-center">
      <div className="border-border bg-card text-foreground flex h-7 max-w-56 shrink-0 items-center overflow-hidden rounded-full border">
        <button
          type="button"
          data-tauri-drag-region="false"
          aria-label={action.label}
          title={action.title}
          disabled={disabled}
          onClick={action.onClick}
          className={cn([
            "flex h-full min-w-0 items-center gap-1.5 py-0 pr-1.5 pl-1.5",
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
                "text-muted-foreground flex h-full w-5 shrink-0 items-center justify-center",
                "hover:bg-accent hover:text-foreground transition-colors",
                open && "bg-accent text-foreground",
              ])}
            >
              <CaretDown size={14} />
            </button>
          )}
        />
      </div>
      {showCountdown ? (
        <div
          data-header-meeting-countdown
          className="border-border bg-popover text-popover-foreground pointer-events-none absolute top-full left-1/2 z-20 mt-2 -translate-x-1/2 rounded-md border px-2.5 py-1 font-mono text-xs whitespace-nowrap tabular-nums shadow-sm"
        >
          <span
            data-header-meeting-countdown-tail
            aria-hidden="true"
            className="border-border bg-popover absolute -top-1.5 left-1/2 size-3 -translate-x-1/2 rotate-45 border-t border-l"
          />
          <span className="relative">{countdown.label}</span>
        </div>
      ) : null}
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
        icon: <VideoCamera size={18} />,
      };
    default:
      return {
        name: "Meeting",
        icon: <Headset size={18} />,
      };
  }
}
