import { useQuery } from "@tanstack/react-query";
import {
  LoaderCircleIcon,
  PauseIcon,
  PlayIcon,
  Volume2Icon,
} from "lucide-react";
import { useMemo, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import type { SharedAttachmentResolver } from "@/components/shared-note-document";
import {
  createSharedNoteWaveform,
  formatSharedNotePlaybackTime,
} from "@/lib/shared-note-presentation";
import type {
  SharedNoteAttachment,
  SharedNoteAttachmentDownload,
} from "@/lib/shared-notes";

export function SharedNoteAudioPlayer({
  attachment,
  resolve,
}: {
  attachment: SharedNoteAttachment;
  resolve?: SharedAttachmentResolver;
}) {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [playing, setPlaying] = useState(false);
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
  const download = isMatchingDownload(attachment, downloadQuery.data)
    ? downloadQuery.data
    : null;
  const progress = duration > 0 ? currentTime / duration : 0;

  const togglePlayback = async () => {
    const audio = audioRef.current;
    if (!audio || !download) return;
    if (audio.paused) {
      await audio.play().catch(() => setPlaying(false));
      return;
    }
    audio.pause();
  };

  return (
    <section
      aria-label={`Audio recording: ${attachment.filename}`}
      className={cn([
        "mb-6 flex min-w-0 items-center gap-2 rounded-[22px] border border-stone-200 bg-white/80 p-1.5 pr-2",
        "text-stone-600",
      ])}
    >
      {download ? (
        <audio
          ref={audioRef}
          className="hidden"
          aria-hidden="true"
          src={download.signedUrl}
          preload="metadata"
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
          onPlay={() => setPlaying(true)}
          onPause={() => setPlaying(false)}
          onEnded={() => setPlaying(false)}
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
        disabled={!download}
        onClick={() => void togglePlayback()}
      >
        {downloadQuery.isPending && resolve ? (
          <LoaderCircleIcon
            className="size-3.5 animate-spin"
            aria-hidden="true"
          />
        ) : playing ? (
          <PauseIcon className="size-3.5 fill-current" aria-hidden="true" />
        ) : (
          <PlayIcon
            className="ml-0.5 size-3.5 fill-current"
            aria-hidden="true"
          />
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
          disabled={!download || duration === 0}
          onChange={(event) => {
            const next = Number(event.target.value);
            if (!audioRef.current || !Number.isFinite(next)) return;
            audioRef.current.currentTime = next;
            setCurrentTime(next);
          }}
        />
      </div>
      {!resolve ? (
        <Volume2Icon
          className="size-3.5 shrink-0 text-stone-400"
          aria-hidden="true"
        />
      ) : null}
    </section>
  );
}

function isMatchingDownload(
  attachment: SharedNoteAttachment,
  download: SharedNoteAttachmentDownload | null | undefined,
): download is SharedNoteAttachmentDownload {
  return Boolean(
    download &&
    download.id === attachment.id &&
    download.filename === attachment.filename &&
    download.contentType === attachment.contentType &&
    download.sizeBytes === attachment.sizeBytes &&
    download.sha256 === attachment.sha256,
  );
}
