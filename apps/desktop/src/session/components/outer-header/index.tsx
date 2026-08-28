import { useLingui } from "@lingui/react/macro";
import { Headset, Square, VideoCamera } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useRef, useState } from "react";

import {
  colors,
  fonts,
  radii,
  shadows,
} from "@anlg/design-system/tokens.stylex";
import { commands as deeplinkCommands } from "@anlg/plugin-deeplink2";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@anlg/ui/components/ui/popover";
import { safeParseDate } from "@anlg/utils";

import { FolderPicker } from "../folder-picker";
import { TranscriptEditButton } from "../note-input/transcript";
import { RecordingIcon, useHasTranscript } from "../shared";
import { TitleInput } from "../title-input";
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
import { useMeetingAccessibilityActive } from "~/session/hooks/useMeetingAccessibilityActive";
import {
  getRemoteMeeting,
  type RemoteMeeting,
} from "~/session/hooks/useRemoteMeeting";
import { useSessionEvent } from "~/session/hooks/useSessionEvent";
import { useWindowControlsGutter } from "~/shared/hooks/useWindowControlsGutter";
import { getScheme } from "~/shared/utils";
import type { EditorView, Tab } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";
import { useStartListening } from "~/stt/useStartListening";
import {
  isMainWebviewWindow,
  requestMainListenerControl,
} from "~/stt/window-control";

export function OuterHeader({
  sessionId,
  currentView,
  tab,
  standaloneWindow = false,
  viewSwitcher,
  transcriptEditMode = false,
  onTranscriptEditModeChange,
}: {
  sessionId: string;
  currentView: EditorView;
  tab?: Extract<Tab, { type: "sessions" }>;
  standaloneWindow?: boolean;
  viewSwitcher?: React.ReactNode;
  transcriptEditMode?: boolean;
  onTranscriptEditModeChange?: (editMode: boolean) => void;
}) {
  const { leftsidebar } = useShell();
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const sessionEvent = useSessionEvent(sessionId);
  const hasTranscript = useHasTranscript(sessionId);
  const { audioExists } = useAudioPlayer();
  const now = useNow();
  const showWindowControlsGutter = useWindowControlsGutter();
  const showSidebarTimelineHeaderGutter =
    !standaloneWindow && !leftsidebar.expanded;
  const endedAt = sessionEvent?.ended_at
    ? safeParseDate(sessionEvent.ended_at)
    : null;
  const ended = !!endedAt && endedAt.getTime() <= now.getTime();
  const isRecording =
    sessionMode === "active" || sessionMode === "running_batch";
  const isLiveMeeting = isRecording || sessionMode === "finalizing";
  const meetingOver = !isRecording && (ended || hasTranscript || audioExists);
  const showTitleInput = Boolean(tab) && !isLiveMeeting && !meetingOver;

  return (
    <div
      data-tauri-drag-region
      {...stylex.props(
        styles.root,
        standaloneWindow &&
          (showWindowControlsGutter
            ? styles.windowControlsPadding
            : styles.defaultPadding),
        !standaloneWindow && leftsidebar.expanded && styles.defaultPadding,
        showSidebarTimelineHeaderGutter &&
          (showWindowControlsGutter
            ? styles.sidebarWindowControlsPadding
            : styles.sidebarPadding),
      )}
    >
      {viewSwitcher}
      {showTitleInput && tab ? (
        <div {...stylex.props(styles.titleContainer)}>
          <TitleInput key={tab.id} tab={tab} variant="breadcrumb" />
        </div>
      ) : null}
      <div
        data-tauri-drag-region
        data-session-header-spacer
        {...stylex.props(styles.spacer)}
      />
      <div data-tauri-drag-region {...stylex.props(styles.actions)}>
        <HeaderMeetingControl
          sessionId={sessionId}
          sessionMode={sessionMode}
          currentView={currentView}
          transcriptEditMode={transcriptEditMode}
          onTranscriptEditModeChange={onTranscriptEditModeChange}
        />
        <FolderPicker sessionId={sessionId} align="end" />
        <MetadataButton sessionId={sessionId} />
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
}: {
  sessionId: string;
  sessionMode: string;
  currentView: EditorView;
  transcriptEditMode: boolean;
  onTranscriptEditModeChange?: (editMode: boolean) => void;
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
      <TranscriptEditButton
        editMode={transcriptEditMode}
        onEditModeChange={onTranscriptEditModeChange}
      />
    );
  }

  const isRecording =
    sessionMode === "active" || sessionMode === "running_batch";

  if (sessionMode === "finalizing") {
    return null;
  }

  if (!sessionEvent && !isRecording) {
    if (hasTranscript || audioExists) {
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
    return null;
  }

  if (ended && !isRecording) {
    return null;
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
  const meetingAccessibilityActive = useMeetingAccessibilityActive(
    canJoinFromHeader && !isWelcomeDemo && sessionMode === "inactive",
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
      url = buildWelcomeNoteDemoUrl(meetingLink);
      try {
        const scheme = await getScheme();
        const result = await deeplinkCommands.startCallbackServer(scheme, null);
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
        icon: <Square {...stylex.props(styles.stopIcon)} weight="fill" />,
        onClick: stopListening,
      };
    }

    if (sessionMode === "running_batch") {
      return {
        label: t`Stop`,
        title: t`Stop transcription`,
        icon: <Square {...stylex.props(styles.stopIcon)} weight="fill" />,
        onClick: () => {
          void stopTranscription(sessionId);
        },
      };
    }

    if (canJoinFromHeader && !meetingAccessibilityActive) {
      return {
        label: t`Join & record`,
        title: t`Join meeting and record`,
        icon: isWelcomeDemo ? (
          <img
            src="/assets/anarlog-icon.png"
            alt=""
            {...stylex.props(styles.meetingIcon)}
          />
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
  const isPrimaryCta = sessionMode === "inactive";
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
      <div {...stylex.props(styles.meetingControl)}>
        <PopoverAnchor asChild>
          <button
            type="button"
            data-tauri-drag-region="false"
            aria-label={action.label}
            title={action.title}
            disabled={disabled}
            onClick={action.onClick}
            {...stylex.props(
              styles.meetingButton,
              isPrimaryCta
                ? styles.primaryMeetingButton
                : styles.secondaryMeetingButton,
              !disabled &&
                (isPrimaryCta
                  ? styles.primaryMeetingButtonEnabled
                  : styles.secondaryMeetingButtonEnabled),
              disabled && styles.meetingButtonDisabled,
            )}
          >
            {action.icon}
            <span {...stylex.props(styles.truncate)}>{action.label}</span>
          </button>
        </PopoverAnchor>
        {showWelcomeDemoPrompt ? (
          <PopoverContent
            data-welcome-demo-prompt
            side="bottom"
            sideOffset={10}
            onOpenAutoFocus={(event) => event.preventDefault()}
            sx={styles.welcomePrompt}
          >
            <span
              data-welcome-demo-prompt-tail
              aria-hidden="true"
              {...stylex.props(styles.promptTail)}
            />
            <span {...stylex.props(styles.promptTitle)}>{t`Try the demo`}</span>
            <span {...stylex.props(styles.promptDescription)}>
              {t`This is a prerecorded demo, so your camera stays off. Click Join & record to see Anarlog in action.`}
            </span>
          </PopoverContent>
        ) : showCountdown ? (
          <div
            data-header-meeting-countdown
            {...stylex.props(styles.countdown)}
          >
            <span
              data-header-meeting-countdown-tail
              aria-hidden="true"
              {...stylex.props(styles.promptTail)}
            />
            <span {...stylex.props(styles.relative)}>{countdown.label}</span>
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
        icon: (
          <img
            src="/assets/zoom-icon.svg"
            alt=""
            {...stylex.props(styles.meetingIcon)}
          />
        ),
      };
    case "google-meet":
      return {
        name: "Meet",
        icon: (
          <img
            src="/assets/google-meet.svg"
            alt=""
            {...stylex.props(styles.meetingIcon)}
          />
        ),
      };
    case "webex":
      return {
        name: "Webex",
        icon: (
          <img
            src="/assets/webex.png"
            alt=""
            {...stylex.props(styles.meetingIcon)}
          />
        ),
      };
    case "teams":
      return {
        name: "Teams",
        icon: (
          <img
            src="/assets/teams.png"
            alt=""
            {...stylex.props(styles.meetingIcon)}
          />
        ),
      };
    case "cal-com":
      return {
        name: "Cal.com",
        icon: <VideoCamera {...stylex.props(styles.meetingIcon)} />,
      };
    default:
      return {
        name: "Meeting",
        icon: <Headset {...stylex.props(styles.meetingIcon)} />,
      };
  }
}

const styles = stylex.create({
  actions: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    paddingRight: "0.25rem",
    position: "relative",
    zIndex: 10,
  },
  countdown: {
    backgroundColor: colors.popover,
    borderColor: colors.border,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.sm,
    color: colors.popoverForeground,
    fontFamily: fonts.mono,
    fontSize: "0.75rem",
    fontVariantNumeric: "tabular-nums",
    left: "50%",
    marginTop: "0.5rem",
    paddingBlock: "0.25rem",
    paddingInline: "0.625rem",
    pointerEvents: "none",
    position: "absolute",
    top: "100%",
    transform: "translateX(-50%)",
    whiteSpace: "nowrap",
    zIndex: 20,
  },
  defaultPadding: {
    paddingLeft: "0.5rem",
  },
  meetingButton: {
    alignItems: "center",
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    flexShrink: 0,
    fontSize: "0.875rem",
    fontWeight: 500,
    gap: "0.375rem",
    height: "1.75rem",
    maxWidth: "14rem",
    overflow: "hidden",
    paddingLeft: "0.375rem",
    paddingRight: "0.625rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color, border-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  meetingButtonDisabled: {
    cursor: "default",
    opacity: 0.6,
  },
  meetingControl: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    marginRight: "0.25rem",
    minWidth: 0,
    position: "relative",
  },
  meetingIcon: {
    flexShrink: 0,
    height: "0.875rem",
    width: "0.875rem",
  },
  primaryMeetingButton: {
    backgroundColor: colors.primary,
    borderColor: colors.primary,
    boxShadow: shadows.sm,
    color: colors.primaryForeground,
  },
  primaryMeetingButtonEnabled: {
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in srgb, ${colors.primary} 90%, transparent)`,
    },
  },
  promptDescription: {
    color: colors.mutedForeground,
    display: "block",
    lineHeight: 1.375,
    marginTop: "0.125rem",
    position: "relative",
  },
  promptTail: {
    backgroundColor: colors.popover,
    borderLeftColor: colors.border,
    borderLeftStyle: "solid",
    borderLeftWidth: "1px",
    borderTopColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    height: "0.75rem",
    left: "50%",
    position: "absolute",
    top: "-0.375rem",
    transform: "translateX(-50%) rotate(45deg)",
    width: "0.75rem",
  },
  promptTitle: {
    display: "block",
    fontWeight: 500,
    position: "relative",
  },
  relative: {
    position: "relative",
  },
  root: {
    alignItems: "center",
    display: "flex",
    gap: "2px",
    height: "3rem",
    position: "relative",
    width: "100%",
  },
  secondaryMeetingButton: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    color: colors.foreground,
  },
  secondaryMeetingButtonEnabled: {
    backgroundColor: {
      default: colors.card,
      ":hover": colors.accent,
    },
  },
  sidebarPadding: {
    paddingLeft: "32px",
  },
  sidebarWindowControlsPadding: {
    paddingLeft: "108px",
  },
  spacer: {
    flex: "1",
    minHeight: "100%",
    minWidth: 0,
  },
  stopIcon: {
    color: "#ef4444",
    height: "0.75rem",
    width: "0.75rem",
  },
  titleContainer: {
    flexShrink: 1,
    maxWidth: "14rem",
    minWidth: 0,
  },
  truncate: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  welcomePrompt: {
    backgroundColor: colors.popover,
    borderColor: colors.border,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.sm,
    color: colors.popoverForeground,
    fontSize: "0.875rem",
    maxWidth: "calc(100vw - 1rem)",
    paddingBlock: "0.625rem",
    paddingInline: "0.75rem",
    pointerEvents: "none",
    width: "18rem",
  },
  windowControlsPadding: {
    paddingLeft: "76px",
  },
});

export { styles as outerHeaderStyles };
