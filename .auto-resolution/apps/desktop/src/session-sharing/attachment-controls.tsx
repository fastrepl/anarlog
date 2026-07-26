import { useMutation } from "@tanstack/react-query";
import {
  AlertCircleIcon,
  CloudIcon,
  Loader2Icon,
  PaperclipIcon,
  RefreshCwIcon,
} from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

import {
  isAttachmentShareable,
  type SessionShareAttachment,
} from "./attachments";

import { retryAttachmentTransfersForAttachment } from "~/attachment-sync/store";
import { useConfigValue } from "~/shared/config";

export function SessionAttachmentControls({
  attachments,
  sharedAttachmentIds,
  canUseCloud,
  canInclude,
  cloudPendingAttachmentId,
  sharePendingAttachmentId,
  onCloudChange,
  onShareChange,
}: {
  attachments: SessionShareAttachment[];
  sharedAttachmentIds: Map<string, string>;
  canUseCloud: boolean;
  canInclude: boolean;
  cloudPendingAttachmentId: string | null;
  sharePendingAttachmentId: string | null;
  onCloudChange: (attachment: SessionShareAttachment, enabled: boolean) => void;
  onShareChange: (
    attachment: SessionShareAttachment,
    included: boolean,
  ) => void;
}) {
  const cloudSyncEnabled = useConfigValue("cloud_sync_enabled");
  const retryMutation = useMutation({
    mutationFn: retryAttachmentTransfersForAttachment,
    onError: () => {
      sonnerToast.error("Could not retry this attachment transfer.");
    },
  });

  if (attachments.length === 0) return null;

  return (
    <section
      aria-labelledby="share-attachments-heading"
      className="border-border/60 border-t pt-5"
    >
      <div className="mb-3">
        <h3 id="share-attachments-heading" className="text-sm font-medium">
          Attachments
        </h3>
        <p className="text-muted-foreground mt-0.5 text-xs">
          Back up a file to Private cloud first, then choose whether to include
          it in this shared note.
        </p>
      </div>
      <div className="space-y-2">
        {attachments.map((attachment) => {
          const operationPending = Boolean(
            cloudPendingAttachmentId || sharePendingAttachmentId,
          );
          const cloudPending = cloudPendingAttachmentId === attachment.id;
          const sharePending = sharePendingAttachmentId === attachment.id;
          const failed = attachment.transferPhase === "failed";
          const included = sharedAttachmentIds.has(attachment.id);
          return (
            <div
              key={attachment.id}
              className="border-border/60 rounded-xl border px-3 py-3"
            >
              <div className="flex items-start gap-2.5">
                <div className="bg-muted mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-lg">
                  {attachment.sourceType === "session_audio" ? (
                    <CloudIcon className="size-3.5" aria-hidden="true" />
                  ) : (
                    <PaperclipIcon className="size-3.5" aria-hidden="true" />
                  )}
                </div>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-xs font-medium">
                    {attachment.filename}
                  </p>
                  <p className="text-muted-foreground mt-0.5 text-[11px]">
                    {formatBytes(attachment.sizeBytes)} ·{" "}
                    {transferStatus(attachment)}
                  </p>
                  {failed && attachment.transferError ? (
                    <p className="text-destructive mt-1 line-clamp-2 text-[11px]">
                      {attachment.transferError}
                    </p>
                  ) : null}
                </div>
                {failed ? (
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    aria-label={`Retry ${attachment.filename}`}
                    disabled={retryMutation.isPending}
                    onClick={() => retryMutation.mutate(attachment.id)}
                    className="size-7 shrink-0"
                  >
                    {retryMutation.isPending &&
                    retryMutation.variables === attachment.id ? (
                      <Loader2Icon className="size-3.5 animate-spin" />
                    ) : (
                      <RefreshCwIcon className="size-3.5" />
                    )}
                  </Button>
                ) : null}
              </div>
              <div className="mt-2.5 grid grid-cols-2 gap-2">
                <Select
                  value={attachment.cloudSyncEnabled ? "cloud" : "local"}
                  disabled={
                    operationPending ||
                    (!attachment.cloudSyncEnabled &&
                      (!canUseCloud || !cloudSyncEnabled))
                  }
                  onValueChange={(value) =>
                    onCloudChange(attachment, value === "cloud")
                  }
                >
                  <SelectTrigger
                    aria-label={`Storage for ${attachment.filename}`}
                    className="h-8 rounded-full text-xs"
                  >
                    {cloudPending ? (
                      <Loader2Icon className="size-3 animate-spin" />
                    ) : null}
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="local">Only on this device</SelectItem>
                    <SelectItem
                      value="cloud"
                      disabled={!canUseCloud || !cloudSyncEnabled}
                    >
                      Private cloud
                    </SelectItem>
                  </SelectContent>
                </Select>
                <Select
                  value={included ? "included" : "private"}
                  disabled={
                    operationPending ||
                    (!included &&
                      (!canInclude || !isAttachmentShareable(attachment)))
                  }
                  onValueChange={(value) =>
                    onShareChange(attachment, value === "included")
                  }
                >
                  <SelectTrigger
                    aria-label={`Sharing for ${attachment.filename}`}
                    className="h-8 rounded-full text-xs"
                  >
                    {sharePending ? (
                      <Loader2Icon className="size-3 animate-spin" />
                    ) : null}
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="private">Not shared</SelectItem>
                    <SelectItem value="included">
                      Include in shared note
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
              {!cloudSyncEnabled ? (
                <p className="text-muted-foreground mt-2 text-[11px]">
                  Turn on Cloud Sync in Settings to use private cloud storage.
                </p>
              ) : attachment.localAvailability !== "present" ? (
                <p className="text-muted-foreground mt-2 flex items-center gap-1 text-[11px]">
                  <AlertCircleIcon className="size-3" aria-hidden="true" />
                  This file is not available on this device yet.
                </p>
              ) : attachment.cloudSyncEnabled && !attachment.cloudObjectKey ? (
                <p className="text-muted-foreground mt-2 flex items-center gap-1 text-[11px]">
                  <Loader2Icon
                    className="size-3 animate-spin"
                    aria-hidden="true"
                  />
                  Wait for the private backup before sharing this file.
                </p>
              ) : null}
            </div>
          );
        })}
      </div>
    </section>
  );
}

function transferStatus(attachment: SessionShareAttachment) {
  if (attachment.transferPhase === "failed") return "Needs attention";
  if (attachment.transferPhase === "retry_wait") return "Retry scheduled";
  if (
    attachment.transferPhase &&
    !["completed", "queued"].includes(attachment.transferPhase)
  ) {
    if (attachment.transferDirection === "download") return "Downloading…";
    if (attachment.transferDirection === "delete")
      return "Removing from cloud…";
    return "Uploading…";
  }
  if (attachment.transferPhase === "queued") return "Waiting to transfer";
  return attachment.cloudObjectKey
    ? "Available in private cloud"
    : "Local only";
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${Math.round(value / 1024)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}
