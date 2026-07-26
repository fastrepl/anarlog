import { useQueries } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback, useMemo } from "react";

import { and, eq, sharedSessionAttachmentCache } from "@hypr/db";
import type { AttachmentResolver } from "@hypr/editor/node-views";

import { attachmentTransferNative } from "~/attachment-sync/native";
import { db, useDrizzleLiveQuery } from "~/db";
import { sharedAttachmentCacheStore } from "~/shared-notes/attachment-cache-store";

type SharedAttachmentPathRow = {
  attachment_id: string;
  sha256: string;
  availability: string;
  access_version: number;
  cache_generation: number;
};

type SharedAttachmentPath = {
  attachmentId: string;
  sha256: string;
  availability: string;
  accessVersion: number;
  cacheGeneration: number;
};

export function useSharedAttachmentResolver(
  viewerUserId: string,
  shareId: string,
): AttachmentResolver {
  const query = db
    .select({
      attachmentId: sharedSessionAttachmentCache.attachmentId,
      sha256: sharedSessionAttachmentCache.sha256,
      availability: sharedSessionAttachmentCache.availability,
      accessVersion: sharedSessionAttachmentCache.accessVersion,
      cacheGeneration: sharedSessionAttachmentCache.cacheGeneration,
    })
    .from(sharedSessionAttachmentCache)
    .where(
      and(
        eq(sharedSessionAttachmentCache.viewerUserId, viewerUserId),
        eq(sharedSessionAttachmentCache.shareId, shareId),
      ),
    );
  const { data = [] } = useDrizzleLiveQuery<
    SharedAttachmentPathRow,
    SharedAttachmentPath[]
  >(query, {
    enabled: Boolean(viewerUserId && shareId),
    mapRows: (rows) =>
      rows.map((row) => ({
        attachmentId: row.attachment_id,
        sha256: row.sha256,
        availability: row.availability,
        accessVersion: row.access_version,
        cacheGeneration: row.cache_generation,
      })),
  });
  const present = data.filter((row) => row.availability === "present");
  const pathQueries = useQueries({
    queries: present.map((row) => ({
      queryKey: [
        "shared-attachment-path",
        viewerUserId,
        shareId,
        row.attachmentId,
        row.sha256,
        row.cacheGeneration,
      ],
      queryFn: async () => {
        const path = await attachmentTransferNative.sharedAttachmentPath(
          viewerUserId,
          row.attachmentId,
        );
        if (!path) {
          await sharedAttachmentCacheStore.markMissing(
            viewerUserId,
            shareId,
            row.attachmentId,
            row.sha256,
            row.accessVersion,
          );
          return null;
        }
        return { path, src: convertFileSrc(path) };
      },
      retry: false,
      staleTime: Infinity,
    })),
  });
  const paths = useMemo(
    () =>
      new Map(
        present.flatMap((row, index) => {
          const resolution = pathQueries[index]?.data;
          return resolution ? ([[row.attachmentId, resolution]] as const) : [];
        }),
      ),
    [pathQueries, present],
  );
  return useCallback(
    (attachmentId: string) => paths.get(attachmentId) ?? null,
    [paths],
  );
}
