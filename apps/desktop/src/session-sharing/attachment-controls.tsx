import { t } from "@lingui/core/macro";
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
  if (!included && audio.localAvailability !== "present") return null;

  const pending = pendingAttachmentId === audio.id;
  const available = isAttachmentShareable(audio);

  return (
    <section className="border-border/60 border-t py-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2.5">
          <div className="bg-muted flex size-7 shrink-0 items-center justify-center rounded-lg">
            <Waveform className="size-4" aria-hidden="true" />
          </div>
          <div className="min-w-0">
            <label
              htmlFor={`share-audio-${audio.id}`}
              className="text-xs font-medium"
            >
              {t`Share audio`}
            </label>
            <p className="text-muted-foreground text-[11px]">
              {available || included
                ? t`Let people with access play the recording.`
                : t`Audio is not available on this device.`}
            </p>
          </div>
        </div>
        {pending ? (
          <CircleNotch
            aria-label={t`Updating audio sharing`}
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
