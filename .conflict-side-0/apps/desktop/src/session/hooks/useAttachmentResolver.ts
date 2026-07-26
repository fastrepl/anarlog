import { useQuery } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback } from "react";

import type { AttachmentResolver } from "@hypr/editor/node-views";
import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

export function sessionAttachmentPathsQueryKey(sessionId: string) {
  return ["session", sessionId, "attachment-paths"] as const;
}

export function useAttachmentResolver(sessionId: string): AttachmentResolver {
  const { data = EMPTY_ATTACHMENTS } = useQuery({
    queryKey: sessionAttachmentPathsQueryKey(sessionId),
    queryFn: async () => {
      const result = await fsSyncCommands.attachmentList(sessionId);
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return new Map(
        result.data.map((attachment) => [
          attachment.attachmentId,
          {
            path: attachment.path,
            src: convertFileSrc(attachment.path),
          },
        ]),
      );
    },
    retry: false,
  });

  return useCallback(
    (attachmentId: string) => data.get(attachmentId) ?? null,
    [data],
  );
}

const EMPTY_ATTACHMENTS = new Map<string, { path: string; src: string }>();
