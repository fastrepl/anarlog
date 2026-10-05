import { useMutation } from "@tanstack/react-query";
import { File, Paths } from "expo-file-system";
import { useRef } from "react";
import { Text, View } from "react-native";

import { useAuth } from "@/auth/context";
import { NoteAttachmentCard } from "@/components/note-attachment-card";
import { Spacing, Typography } from "@/constants/theme";
import type { NoteAttachment } from "@/data/note-attachment-catalog";
import {
  restoreNoteAttachmentFromCloud,
  shareNoteAttachment,
} from "@/data/restore-note-attachment";
import { captureAnalytics } from "@/lib/analytics";
import { env } from "@/lib/env";
import { captureOperationalError } from "@/lib/error-reporting";
import { useMountEffect } from "@/lib/use-mount-effect";
import { createStyleHook } from "@/settings/theme-provider";

export function NoteFiles({
  sessionId,
  attachments,
}: {
  sessionId: string;
  attachments: NoteAttachment[];
}) {
  const styles = useStyles();
  const auth = useAuth();
  const controllerRef = useRef<AbortController | null>(null);
  const mutation = useMutation({
    mutationFn: async ({
      attachment,
      uri,
      controller,
    }: {
      attachment: NoteAttachment;
      uri?: string;
      controller: AbortController;
    }) => {
      try {
        if (controller.signal.aborted) return;
        if (uri) {
          await shareNoteAttachment(uri, attachment);
          captureAnalytics("file_shared", {
            entry_point: "mobile_note_attachment",
            file_type: "attachment",
          });
        } else {
          const accessToken = auth.session?.access_token;
          if (!accessToken || !env.supabaseUrl) return;
          await restoreNoteAttachmentFromCloud(sessionId, attachment, {
            accessToken,
            apiBaseUrl: env.apiUrl,
            supabaseUrl: env.supabaseUrl,
            signal: controller.signal,
          });
          captureAnalytics("file_downloaded", {
            entry_point: "mobile_note_attachment",
            file_type: "attachment",
            size_bytes: attachment.sizeBytes,
          });
        }
      } catch (error) {
        if (controller.signal.aborted) return;
        captureOperationalError(error, {
          operation: uri
            ? "note_attachment_share"
            : "note_attachment_cloud_restore",
        });
        throw new Error(
          error instanceof Error
            ? error.message
            : uri
              ? "The file could not be shared."
              : "The file could not be downloaded to this phone.",
        );
      } finally {
        if (controllerRef.current === controller) controllerRef.current = null;
      }
    },
  });
  useMountEffect(() => () => controllerRef.current?.abort());

  const run = (attachment: NoteAttachment, uri?: string) => {
    if (controllerRef.current) return;
    if (
      !uri &&
      (!attachment.cloudObjectKey ||
        !auth.session?.access_token ||
        !env.supabaseUrl)
    )
      return;
    const controller = new AbortController();
    controllerRef.current = controller;
    mutation.mutate({ attachment, uri, controller });
  };

  if (!attachments.length) return null;
  return (
    <View style={styles.attachments}>
      <Text style={styles.label}>Files</Text>
      {attachments.map((attachment) => {
        const candidate = attachment.localRelativePath
          ? new File(
              Paths.document,
              "sessions",
              sessionId,
              attachment.localRelativePath,
            )
          : null;
        const file = candidate?.exists === true ? candidate : null;
        const selected =
          mutation.variables?.attachment.attachmentId ===
          attachment.attachmentId;
        return (
          <NoteAttachmentCard
            key={attachment.attachmentId}
            availableLocally={file !== null}
            cloudAvailable={Boolean(
              attachment.cloudObjectKey &&
              auth.billing.isPro &&
              auth.session?.access_token &&
              env.supabaseUrl,
            )}
            errorMessage={selected ? (mutation.error?.message ?? null) : null}
            filename={attachment.filename}
            loading={selected && mutation.isPending}
            onDownload={() => run(attachment)}
            onShare={() => {
              if (file) run(attachment, file.uri);
            }}
            sizeBytes={attachment.sizeBytes}
          />
        );
      })}
    </View>
  );
}

const useStyles = createStyleHook((Colors) => ({
  attachments: {
    gap: Spacing.sm,
    marginHorizontal: Spacing.md,
    marginTop: Spacing.md,
  },
  label: { ...Typography.captionStrong, color: Colors.muted },
}));
