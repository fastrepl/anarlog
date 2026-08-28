import { Pause, Play } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  colors,
  fonts,
  radii,
  shadows,
} from "@anlg/design-system/tokens.stylex";

import { useAudioPlayer, useAudioTime } from "./provider";
import { TimelineMeta, TimelineShell } from "./timeline-shell";

import { useBillingAccess } from "~/auth/billing-context";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";

const PLAYBACK_RATES = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];

export function Timeline({
  contentClassName,
}: {
  contentClassName?: string;
} = {}) {
  const { isPro } = useBillingAccess();
  const {
    registerContainer,
    state,
    pause,
    resume,
    start,
    stop,
    playbackRate,
    setPlaybackRate,
    deleteRecording,
    isDeletingRecording,
  } = useAudioPlayer();
  const time = useAudioTime();
  const [showRateMenu, setShowRateMenu] = useState(false);
  const rateMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (
        rateMenuRef.current &&
        !rateMenuRef.current.contains(e.target as Node)
      ) {
        setShowRateMenu(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  const handleClick = () => {
    if (state === "playing") {
      pause();
    } else if (state === "paused") {
      resume();
    } else if (state === "stopped") {
      start();
    }
  };

  const handleDeleteRecording = useCallback(async () => {
    setShowRateMenu(false);
    await deleteRecording();
  }, [deleteRecording]);

  const contextMenu = useMemo(
    () => [
      ...(state === "paused"
        ? [{ id: "resume", text: "Resume", action: resume }]
        : []),
      ...(state === "stopped"
        ? [{ id: "play", text: "Play", action: start }]
        : []),
      ...(state === "playing"
        ? [{ id: "pause", text: "Pause", action: pause }]
        : []),
      ...(state !== "stopped"
        ? [{ id: "stop", text: "Stop", action: stop }]
        : []),
      { separator: true as const },
      {
        id: "delete-recording",
        text: "Delete recording",
        action: () => void handleDeleteRecording(),
        disabled: isDeletingRecording,
      },
    ],
    [
      state,
      resume,
      start,
      pause,
      stop,
      isDeletingRecording,
      handleDeleteRecording,
    ],
  );
  const showContextMenu = useNativeContextMenu(contextMenu);

  return (
    <TimelineShell
      contentClassName={contentClassName}
      onContextMenu={showContextMenu}
      leading={
        <button onClick={handleClick} {...stylex.props(styles.playButton)}>
          {state === "playing" ? (
            <Pause {...stylex.props(styles.playIcon)} weight="fill" />
          ) : (
            <Play {...stylex.props(styles.playIcon)} weight="fill" />
          )}
        </button>
      }
      meta={
        <>
          <TimelineMeta>
            <span>{formatTime(time.current)}</span>/
            <span>{formatTime(time.total)}</span>
          </TimelineMeta>

          {isPro ? (
            <div {...stylex.props(styles.rateMenuAnchor)} ref={rateMenuRef}>
              <button
                onClick={() => setShowRateMenu((prev) => !prev)}
                {...stylex.props(styles.rateButton)}
              >
                {playbackRate}x
              </button>
              {showRateMenu && (
                <div {...stylex.props(styles.rateMenu)}>
                  {PLAYBACK_RATES.map((rate) => (
                    <button
                      key={rate}
                      onClick={() => {
                        setPlaybackRate(rate);
                        setShowRateMenu(false);
                      }}
                      {...stylex.props(
                        styles.rateOption,
                        rate === playbackRate
                          ? styles.rateOptionSelected
                          : styles.rateOptionIdle,
                      )}
                    >
                      {rate}x
                    </button>
                  ))}
                </div>
              )}
            </div>
          ) : null}
        </>
      }
      main={<div ref={registerContainer} {...stylex.props(styles.timeline)} />}
    />
  );
}

function formatTime(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
}

const styles = stylex.create({
  playButton: {
    alignItems: "center",
    backgroundColor: {
      default: colors.card,
      ":hover": colors.accent,
    },
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.sm,
    display: "flex",
    flexShrink: 0,
    height: "1.75rem",
    justifyContent: "center",
    transform: {
      default: null,
      ":hover": "scale(1.1)",
    },
    transitionDuration: "150ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    userSelect: "none",
    width: "1.75rem",
  },
  playIcon: {
    color: colors.foreground,
    height: "0.875rem",
    width: "0.875rem",
  },
  rateButton: {
    alignItems: "center",
    backgroundColor: {
      default: colors.card,
      ":hover": colors.accent,
    },
    borderColor: colors.border,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.sm,
    color: colors.mutedForeground,
    display: "flex",
    fontFamily: fonts.mono,
    fontSize: "0.75rem",
    height: "1.5rem",
    justifyContent: "center",
    paddingInline: "0.375rem",
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    userSelect: "none",
  },
  rateMenu: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    bottom: "100%",
    boxShadow:
      "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)",
    marginBottom: "0.25rem",
    paddingBlock: "0.25rem",
    position: "absolute",
    right: 0,
  },
  rateMenuAnchor: {
    flexShrink: 0,
    position: "relative",
  },
  rateOption: {
    backgroundColor: {
      default: null,
      ":hover": colors.accent,
    },
    display: "block",
    fontFamily: fonts.mono,
    fontSize: "0.75rem",
    paddingBlock: "0.25rem",
    paddingInline: "0.75rem",
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    userSelect: "none",
    width: "100%",
  },
  rateOptionIdle: {
    color: colors.mutedForeground,
  },
  rateOptionSelected: {
    color: colors.foreground,
    fontWeight: 600,
  },
  timeline: {
    flex: "1",
    height: "1.5rem",
    minWidth: 0,
    width: "100%",
  },
});
