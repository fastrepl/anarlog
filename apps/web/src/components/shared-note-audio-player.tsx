import { useQuery } from "@tanstack/react-query";
import { useMemo, useRef, useState } from "react";

import {
  CircleNotch,
  Pause,
  Play,
  SpeakerHigh,
} from "@anlg/ui/components/icons";
import { cn } from "@anlg/utils";

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
        className={cn([
          "mb-6 flex min-w-0 items-center gap-3 rounded-[22px] border border-stone-200 bg-white/80 px-3 py-2",
          "text-stone-600",
        ])}
      >
        <SpeakerHigh
          className="size-3.5 shrink-0 text-stone-400"
          aria-hidden="true"
        />
        <span className="min-w-0 flex-1 truncate text-sm">
          Attachment unavailable
        </span>
        <button
          type="button"
          className={cn([
            "shrink-0 rounded-full border border-stone-300 bg-white px-3 py-1 text-xs font-medium text-stone-700 shadow-xs",
            "hover:bg-stone-50",
            "focus-visible:ring-2 focus-visible:ring-stone-900 focus-visible:ring-offset-2 focus-visible:outline-hidden",
          ])}
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
      className={cn([
        "mb-6 flex min-w-0 items-center gap-2 rounded-[22px] border border-stone-200 bg-white/80 p-1.5 pr-2",
        "text-stone-600",
      ])}
    >
      {activeDownload ? (
        <audio
          key={activeDownload.signedUrl}
          ref={audioRef}
          className="hidden"
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
        className={cn([
          "grid size-7 shrink-0 place-items-center rounded-full border border-stone-300 bg-white text-stone-800 shadow-xs",
          "transition-transform hover:scale-105 hover:bg-stone-50",
          "focus-visible:ring-2 focus-visible:ring-stone-900 focus-visible:ring-offset-2 focus-visible:outline-hidden",
          "disabled:cursor-default disabled:opacity-50",
        ])}
        disabled={!activeDownload}
        onClick={() => void togglePlayback()}
      >
        {downloadQuery.isPending && resolve ? (
          <CircleNotch className="size-3.5 animate-spin" aria-hidden="true" />
        ) : playing ? (
          <Pause className="size-3.5" weight="fill" aria-hidden="true" />
        ) : (
          <Play className="ml-0.5 size-3.5" weight="fill" aria-hidden="true" />
        )}
      </button>
      <span className="flex shrink-0 gap-1 font-mono text-[10px] tabular-nums">
        <span>{formatSharedNotePlaybackTime(currentTime)}</span>
        <span aria-hidden="true">/</span>
        <span>{formatSharedNotePlaybackTime(duration)}</span>
      </span>
      <div className="relative flex h-6 min-w-0 flex-1 items-center gap-0.5 overflow-hidden">
        {waveform.map((height, index) => (
          <span
            key={index}
            aria-hidden="true"
            className={cn([
              "min-h-0.5 min-w-px flex-1 rounded-full",
              index / waveform.length <= progress
                ? "bg-stone-600"
                : "bg-stone-300",
            ])}
            style={{ height: `${height}%` }}
          />
        ))}
        <input
          type="range"
          aria-label="Recording position"
          className="absolute inset-0 size-full cursor-pointer opacity-0"
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
        <SpeakerHigh
          className="size-3.5 shrink-0 text-stone-400"
          aria-hidden="true"
        />
      ) : null}
    </section>
  );
}
