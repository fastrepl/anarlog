import { CircleNotch, Waveform } from "@phosphor-icons/react";

import { Switch } from "@anlg/ui/components/ui/switch";

import {
  isAttachmentShareable,
  type SessionShareAttachment,
} from "./attachments";

export function SessionAttachmentControls({
  attachments,
  sharedAttachmentIds,
  canShare,
  pendingAttachmentId,
  onShareChange,
}: {
  attachments: SessionShareAttachment[];
  sharedAttachmentIds: Map<string, string>;
  canShare: boolean;
  pendingAttachmentId: string | null;
  onShareChange: (
    attachment: SessionShareAttachment,
    included: boolean,
  ) => void;
}) {
  const audio = attachments.find(
    (attachment) => attachment.sourceType === "session_audio",
  );
  if (!audio) return null;

  const included = sharedAttachmentIds.has(audio.id);
  const pending = pendingAttachmentId === audio.id;
  const available = isAttachmentShareable(audio);

  return (
    <section className="border-border/60 border-t py-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2.5">
          <div className="bg-muted flex size-7 shrink-0 items-center justify-center rounded-lg">
            <Waveform className="size-3.5" aria-hidden="true" />
          </div>
          <div className="min-w-0">
            <label
              htmlFor={`share-audio-${audio.id}`}
              className="text-xs font-medium"
            >
              Share audio
            </label>
            <p className="text-muted-foreground text-[11px]">
              {available || included
                ? "Let people with access play the recording."
                : "Audio is not available on this device."}
            </p>
          </div>
        </div>
        {pending ? (
          <CircleNotch
            aria-label="Updating audio sharing"
            className="text-muted-foreground size-4 animate-spin"
          />
        ) : (
          <Switch
            id={`share-audio-${audio.id}`}
            size="sm"
            checked={included}
            disabled={!canShare || (!included && !available)}
            onCheckedChange={(checked) => onShareChange(audio, checked)}
          />
        )}
      </div>
    </section>
  );
}
