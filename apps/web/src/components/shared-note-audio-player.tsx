import { CircleNotch, Pause, Play, SpeakerHigh } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useQuery } from "@tanstack/react-query";
import { useMemo, useRef, useState } from "react";

import {
  colors,
  fonts,
  radii,
  shadows,
} from "@anlg/design-system/tokens.stylex";

import type { SharedAttachmentResolver } from "@/components/shared-note-document";
import {
  createSharedNoteWaveform,
  formatSharedNotePlaybackTime,
  isSharedNoteAudioGrantExpiring,
} from "@/lib/shared-note-presentation";
import {
  isMatchingSharedNoteAttachmentDownload,
  type SharedNoteAttachment,
  type SharedNoteAttachmentDownload,
} from "@/lib/shared-notes";

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  style1: {
    width: ".875rem",
    height: ".875rem",
    flexShrink: 0,
    color: colors.mutedForeground,
  },
  style2: {
    minWidth: 0,
    flexBasis: "0%",
    flexGrow: 1,
    flexShrink: 1,
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    overflow: "hidden",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
  },
  style3: {
    display: "none",
  },
  style4: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    width: ".875rem",
    height: ".875rem",
  },
  style5: {
    width: ".875rem",
    height: ".875rem",
  },
  style6: {
    marginLeft: ".125rem",
    width: ".875rem",
    height: ".875rem",
  },
  style7: {
    display: "flex",
    flexShrink: 0,
    gap: ".25rem",
    fontFamily: fonts.mono,
    fontSize: "10px",
    fontVariantNumeric: "tabular-nums",
  },
  style8: {
    position: "relative",
    display: "flex",
    height: "1.5rem",
    minWidth: 0,
    flexBasis: "0%",
    flexGrow: 1,
    flexShrink: 1,
    alignItems: "center",
    gap: ".125rem",
    overflow: "hidden",
  },
  style9: {
    position: "absolute",
    inset: 0,
    width: "100%",
    height: "100%",
    cursor: "pointer",
    opacity: 0,
  },
  playerShell: {
    alignItems: "center",
    backgroundColor: `color-mix(in srgb, ${colors.card} 80%, transparent)`,
    borderColor: colors.border,
    borderRadius: "22px",
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.mutedForeground,
    display: "flex",
    marginBottom: "1.5rem",
    minWidth: 0,
  },
  unavailablePlayerShell: {
    gap: ".75rem",
    paddingBlock: ".5rem",
    paddingInline: ".75rem",
  },
  readyPlayerShell: {
    gap: ".5rem",
    paddingBlock: ".375rem",
    paddingLeft: ".375rem",
    paddingRight: ".5rem",
  },
  retryButton: {
    backgroundColor: {
      default: colors.card,
      ":hover": colors.background,
    },
    borderColor: colors.appFloatingBorder,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: shadows.sm,
      ":focus-visible": `0 0 0 2px ${colors.card}, 0 0 0 4px ${colors.ring}, ${shadows.sm}`,
    },
    color: colors.cardForeground,
    flexShrink: 0,
    fontSize: ".75rem",
    fontWeight: 500,
    lineHeight: "1rem",
    outline: {
      default: null,
      ":focus-visible": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus-visible": "2px",
    },
    paddingBlock: ".25rem",
    paddingInline: ".75rem",
  },
  playbackButton: {
    backgroundColor: {
      default: colors.card,
      ":hover": colors.background,
    },
    borderColor: colors.appFloatingBorder,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: shadows.sm,
      ":focus-visible": `0 0 0 2px ${colors.card}, 0 0 0 4px ${colors.ring}, ${shadows.sm}`,
    },
    color: colors.primary,
    cursor: {
      default: null,
      ":disabled": "default",
    },
    display: "grid",
    flexShrink: 0,
    height: "1.75rem",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    outline: {
      default: null,
      ":focus-visible": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus-visible": "2px",
    },
    placeItems: "center",
    transform: {
      default: "scale(1)",
      ":hover": "scale(1.05)",
    },
    transitionDuration: "150ms",
    transitionProperty: "transform",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1.75rem",
  },
  waveformBar: (height: number) => ({
    borderRadius: radii.full,
    flexBasis: "0%",
    flexGrow: 1,
    flexShrink: 1,
    height: `${height}%`,
    minHeight: ".125rem",
    minWidth: "1px",
  }),
  waveformPlayed: {
    backgroundColor: colors.mutedForeground,
  },
  waveformRemaining: {
    backgroundColor: colors.appFloatingBorder,
  },
});
export function SharedNoteAudioPlayer({
  attachment,
  resolve,
}: {
  attachment: SharedNoteAttachment;
  resolve?: SharedAttachmentResolver;
}) {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const pendingPlaybackRef = useRef<{
    currentTime: number;
    resume: boolean;
  } | null>(null);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [pinnedDownload, setPinnedDownload] =
    useState<SharedNoteAttachmentDownload | null>(null);
  const waveform = useMemo(
    () => createSharedNoteWaveform(attachment.sha256),
    [attachment.sha256],
  );
  const downloadQuery = useQuery({
    queryKey: ["shared-note-featured-audio", attachment.id],
    queryFn: ({ signal }) => resolve!(attachment, signal),
    enabled: Boolean(resolve),
    retry: false,
    staleTime: 45_000,
    refetchInterval: playing ? false : 45_000,
    gcTime: 0,
  });
  const download =
    !downloadQuery.error &&
    isMatchingSharedNoteAttachmentDownload(attachment, downloadQuery.data)
      ? downloadQuery.data
      : null;
  const pinnedAudioDownload = isMatchingSharedNoteAttachmentDownload(
    attachment,
    pinnedDownload,
  )
    ? pinnedDownload
    : null;
  const activeDownload = pinnedAudioDownload ?? download;
  const progress = duration > 0 ? currentTime / duration : 0;
  const refreshAudioGrant = async (
    audio: HTMLAudioElement,
    resume: boolean,
  ) => {
    const pendingPlayback = pendingPlaybackRef.current;
    const playbackTime = pendingPlayback?.currentTime ?? audio.currentTime;
    const shouldResume = pendingPlayback?.resume ?? resume;
    pendingPlaybackRef.current = null;
    audio.pause();
    const refreshed = await downloadQuery.refetch();
    if (
      refreshed.isError ||
      !isMatchingSharedNoteAttachmentDownload(attachment, refreshed.data)
    ) {
      setPinnedDownload(null);
      setPlaying(false);
      return;
    }
    setPinnedDownload(refreshed.data);
    if (activeDownload?.signedUrl === refreshed.data.signedUrl) {
      requestAnimationFrame(() => {
        const current = audioRef.current;
        if (!current) return;
        current.currentTime = playbackTime;
        setCurrentTime(playbackTime);
        if (shouldResume) {
          void current.play().catch(() => setPlaying(false));
        }
      });
      return;
    }
    pendingPlaybackRef.current = {
      currentTime: playbackTime,
      resume: shouldResume,
    };
  };
  const togglePlayback = async () => {
    const audio = audioRef.current;
    if (!audio || !activeDownload) return;
    if (audio.paused) {
      await audio.play().catch(() => setPlaying(false));
      return;
    }
    audio.pause();
  };
  if (downloadQuery.error && !activeDownload) {
    return (
      <section
        aria-label={`Audio recording: ${attachment.filename}`}
        {...stylex.props(styles.playerShell, styles.unavailablePlayerShell)}
      >
        <SpeakerHigh {...stylex.props(styles.style1)} aria-hidden="true" />
        <span {...stylex.props(styles.style2)}>Attachment unavailable</span>
        <button
          type="button"
          {...stylex.props(styles.retryButton)}
          disabled={downloadQuery.isFetching}
          onClick={() => void downloadQuery.refetch()}
        >
          {downloadQuery.isFetching ? "Retrying…" : "Retry"}
        </button>
      </section>
    );
  }
  return (
    <section
      aria-label={`Audio recording: ${attachment.filename}`}
      {...stylex.props(styles.playerShell, styles.readyPlayerShell)}
    >
      {activeDownload ? (
        <audio
          key={activeDownload.signedUrl}
          ref={audioRef}
          {...stylex.props(styles.style3)}
          aria-hidden="true"
          src={activeDownload.signedUrl}
          preload="metadata"
          onLoadedMetadata={(event) => {
            const pendingPlayback = pendingPlaybackRef.current;
            if (!pendingPlayback) return;
            pendingPlaybackRef.current = null;
            event.currentTarget.currentTime = pendingPlayback.currentTime;
            setCurrentTime(pendingPlayback.currentTime);
            if (pendingPlayback.resume) {
              void event.currentTarget.play().catch(() => setPlaying(false));
            }
          }}
          onDurationChange={(event) =>
            setDuration(
              Number.isFinite(event.currentTarget.duration)
                ? event.currentTarget.duration
                : 0,
            )
          }
          onTimeUpdate={(event) =>
            setCurrentTime(event.currentTarget.currentTime)
          }
          onPlay={(event) => {
            const current = pinnedAudioDownload ?? download;
            if (!current) {
              event.currentTarget.pause();
              return;
            }
            if (isSharedNoteAudioGrantExpiring(current.expiresAt)) {
              void refreshAudioGrant(event.currentTarget, true);
              return;
            }
            setPinnedDownload(current);
            setPlaying(true);
          }}
          onPause={() => setPlaying(false)}
          onEnded={() => {
            setPlaying(false);
            setPinnedDownload(null);
          }}
          onError={(event) => {
            if (downloadQuery.isFetching) return;
            void refreshAudioGrant(event.currentTarget, playing);
          }}
        />
      ) : null}
      <button
        type="button"
        aria-label={playing ? "Pause recording" : "Play recording"}
        {...stylex.props(styles.playbackButton)}
        disabled={!activeDownload}
        onClick={() => void togglePlayback()}
      >
        {downloadQuery.isPending && resolve ? (
          <CircleNotch {...stylex.props(styles.style4)} aria-hidden="true" />
        ) : playing ? (
          <Pause
            {...stylex.props(styles.style5)}
            weight="fill"
            aria-hidden="true"
          />
        ) : (
          <Play
            {...stylex.props(styles.style6)}
            weight="fill"
            aria-hidden="true"
          />
        )}
      </button>
      <span {...stylex.props(styles.style7)}>
        <span>{formatSharedNotePlaybackTime(currentTime)}</span>
        <span aria-hidden="true">/</span>
        <span>{formatSharedNotePlaybackTime(duration)}</span>
      </span>
      <div {...stylex.props(styles.style8)}>
        {waveform.map((height, index) => (
          <span
            key={index}
            aria-hidden="true"
            {...stylex.props(
              styles.waveformBar(height),
              index / waveform.length <= progress
                ? styles.waveformPlayed
                : styles.waveformRemaining,
            )}
          />
        ))}
        <input
          type="range"
          aria-label="Recording position"
          {...stylex.props(styles.style9)}
          min={0}
          max={duration || 0}
          step={0.1}
          value={Math.min(currentTime, duration || 0)}
          disabled={!activeDownload || duration === 0}
          onChange={(event) => {
            const next = Number(event.target.value);
            if (!audioRef.current || !Number.isFinite(next)) return;
            audioRef.current.currentTime = next;
            setCurrentTime(next);
          }}
        />
      </div>
      {!resolve ? (
        <SpeakerHigh {...stylex.props(styles.style1)} aria-hidden="true" />
      ) : null}
    </section>
  );
}
