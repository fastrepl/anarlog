import { useLingui } from "@lingui/react/macro";
import { CaretDown, Headset, Square, VideoCamera } from "@phosphor-icons/react";
import { useCallback, useRef, useState } from "react";

import { commands as deeplinkCommands } from "@anlg/plugin-deeplink2";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@anlg/ui/components/ui/popover";
import { cn, safeParseDate } from "@anlg/utils";

import { FolderPicker } from "../folder-picker";
import { isMeetingStopAction } from "../note-input/header-stop";
import { TranscriptEditButton } from "../note-input/transcript";
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
  viewSwitcher,
  transcriptEditMode = false,
  onTranscriptEditModeChange,
}: {
  sessionId: string;
  currentView: EditorView;
  standaloneWindow?: boolean;
  viewSwitcher?: React.ReactNode;
  transcriptEditMode?: boolean;
  onTranscriptEditModeChange?: (editMode: boolean) => void;
}) {
  const { leftsidebar } = useShell();
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const showWindowControlsGutter = useWindowControlsGutter();
  const showSidebarTimelineHeaderGutter =
    !standaloneWindow && !leftsidebar.expanded;
  const embedStopInViewSwitcher =
    viewSwitcher != null && isMeetingStopAction(sessionMode);

  return (
    <div
      data-tauri-drag-region
      className={cn([
        "relative flex w-full items-center",
        "h-12",
        standaloneWindow && (showWindowControlsGutter ? "pl-[76px]" : "pl-2"),
        showSidebarTimelineHeaderGutter &&
          (showWindowControlsGutter ? "pl-[156px]" : "pl-[80px]"),
      ])}
    >
      {viewSwitcher}
      <div data-tauri-drag-region className="min-h-full min-w-0 flex-1" />
      <div
        data-tauri-drag-region
        className="relative z-10 flex shrink-0 items-center pr-1"
      >
        <HeaderMeetingControl
          sessionId={sessionId}
          sessionMode={sessionMode}
          currentView={currentView}
          transcriptEditMode={transcriptEditMode}
          onTranscriptEditModeChange={onTranscriptEditModeChange}
          hideStop={embedStopInViewSwitcher}
        />
        <FolderPicker sessionId={sessionId} align="end" />
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
  currentView,
  transcriptEditMode,
  onTranscriptEditModeChange,
  hideStop = false,
}: {
  sessionId: string;
  sessionMode: string;
  currentView: EditorView;
  transcriptEditMode: boolean;
  onTranscriptEditModeChange?: (editMode: boolean) => void;
  hideStop?: boolean;
}) {
  const sessionEvent = useSessionEvent(sessionId);
  const hasTranscript = useHasTranscript(sessionId);
  const { audioExists } = useAudioPlayer();
  const now = useNow();
  const endedAt = sessionEvent?.ended_at
    ? safeParseDate(sessionEvent.ended_at)
    : null;
  const ended = !!endedAt && endedAt.getTime() <= now.getTime();
  const canEditTranscript =
    currentView.type === "transcript" &&
    sessionMode === "inactive" &&
    hasTranscript &&
    (!sessionEvent || ended) &&
    onTranscriptEditModeChange;

  if (canEditTranscript) {
    return (
      <>
        <TranscriptEditButton
          editMode={transcriptEditMode}
          onEditModeChange={onTranscriptEditModeChange}
        />
        <div className="shrink-0">
          <MetadataButton sessionId={sessionId} />
        </div>
      </>
    );
  }

  const isRecording =
    sessionMode === "active" || sessionMode === "running_batch";

  if (hideStop && isRecording) {
    return null;
  }

  if (!sessionEvent && !isRecording) {
    if (hasTranscript || audioExists) {
      return (
        <div className="shrink-0">
          <MetadataButton sessionId={sessionId} />
        </div>
      );
    }

    if (sessionMode === "finalizing") {
      return null;
    }

    return (
      <HeaderMeetingActionPill
        sessionId={sessionId}
        event={null}
        sessionMode={sessionMode}
        hasTranscript={hasTranscript}
        audioExists={audioExists}
      />
    );
  }

  if (
    !isRecording &&
    sessionMode === "inactive" &&
    sessionEvent &&
    (hasTranscript || audioExists)
  ) {
    return (
      <div className="shrink-0">
        <MetadataButton sessionId={sessionId} />
      </div>
    );
  }

  if (ended && !isRecording) {
    return (
      <div className="shrink-0">
        <MetadataButton sessionId={sessionId} />
      </div>
    );
  }

  return (
    <HeaderMeetingActionPill
      sessionId={sessionId}
      event={sessionEvent}
      sessionMode={sessionMode}
      hasTranscript={hasTranscript}
      audioExists={audioExists}
    />
  );
}

function HeaderMeetingActionPill({
  sessionId,
  event,
  sessionMode,
  hasTranscript,
  audioExists,
}: {
  sessionId: string;
  event: {
    meeting_link?: string;
    tracking_id?: string;
  } | null;
  sessionMode: string;
  hasTranscript: boolean;
  audioExists: boolean;
}) {
  const startListening = useStartListening(sessionId);
  const { stop, stopTranscription } = useListener((state) => ({
    stop: state.stop,
    stopTranscription: state.stopTranscription,
  }));
  const remote = getRemoteMeeting(event?.meeting_link);
  const meetingLink = event?.meeting_link || null;
  const isWelcomeDemo = event?.tracking_id === WELCOME_NOTE_TRACKING_ID;
  const canJoinFromHeader = Boolean(
    meetingLink && (remote !== null || isWelcomeDemo),
  );
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
    if (isWelcomeDemo) {
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
  }, [isWelcomeDemo, meetingLink]);
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
  const countdown = useEventCountdown(sessionId);
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

    if (canJoinFromHeader) {
      return {
        label: t`Join & record`,
        title: t`Join meeting and record`,
        icon: isWelcomeDemo ? (
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
  const isJoinAction = canJoinFromHeader && sessionMode === "inactive";
  const showCountdown =
    Boolean(countdown.label) &&
    sessionMode !== "active" &&
    sessionMode !== "running_batch" &&
    sessionMode !== "finalizing";
  const showWelcomeDemoPrompt =
    isWelcomeDemo &&
    sessionMode === "inactive" &&
    !hasTranscript &&
    !audioExists;

  return (
    <Popover open={showWelcomeDemoPrompt}>
      <div className="relative mr-1 flex min-w-0 shrink-0 items-center">
        <PopoverAnchor asChild>
          <div
            className={cn([
              "flex h-7 max-w-56 shrink-0 items-center overflow-hidden rounded-full border",
              isJoinAction
                ? "border-primary bg-primary text-primary-foreground shadow-sm dark:border-white dark:bg-white dark:text-black"
                : "border-border bg-card text-foreground",
            ])}
          >
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
                "transition-colors",
                !disabled &&
                  (isJoinAction
                    ? "hover:bg-primary/90 dark:hover:bg-white/90"
                    : "hover:bg-accent"),
                disabled && "cursor-default opacity-60",
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
                    "flex h-full w-5 shrink-0 items-center justify-center transition-colors",
                    isJoinAction
                      ? "text-primary-foreground/70 hover:bg-primary-foreground/14 hover:text-primary-foreground dark:text-black/70 dark:hover:bg-black/8 dark:hover:text-black"
                      : "text-muted-foreground hover:bg-accent hover:text-foreground",
                    open &&
                      (isJoinAction
                        ? "bg-primary-foreground/14 text-primary-foreground dark:bg-black/8 dark:text-black"
                        : "bg-accent text-foreground"),
                  ])}
                >
                  <CaretDown size={14} />
                </button>
              )}
            />
          </div>
        </PopoverAnchor>
        {showWelcomeDemoPrompt ? (
          <PopoverContent
            data-welcome-demo-prompt
            side="bottom"
            sideOffset={10}
            onOpenAutoFocus={(event) => event.preventDefault()}
            className="border-border bg-popover text-popover-foreground pointer-events-none w-72 max-w-[calc(100vw-1rem)] rounded-md border px-3 py-2.5 text-sm shadow-sm"
          >
            <span
              data-welcome-demo-prompt-tail
              aria-hidden="true"
              className="border-border bg-popover absolute -top-1.5 left-1/2 size-3 -translate-x-1/2 rotate-45 border-t border-l"
            />
            <span className="relative block font-medium">{t`Try the demo`}</span>
            <span className="text-muted-foreground relative mt-0.5 block leading-snug">
              {t`This is a prerecorded demo, so your camera stays off. Click Join & record to see Anarlog in action.`}
            </span>
          </PopoverContent>
        ) : showCountdown ? (
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
    </Popover>
  );
}

function getMeetingDisplay(type: RemoteMeeting["type"]) {
  switch (type) {
    case "zoom":
      return {
        name: "Zoom",
        icon: <img src="/assets/zoom-icon.svg" alt="" width={18} height={18} />,
      };
    case "google-meet":
      return {
        name: "Meet",
        icon: (
          <img src="/assets/google-meet.svg" alt="" width={18} height={18} />
        ),
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
