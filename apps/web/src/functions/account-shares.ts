import { createServerFn } from "@tanstack/react-start";
import { setResponseHeader } from "@tanstack/react-start/server";
import { z } from "zod";

import { getSupabaseServerClient } from "@/functions/supabase";
import {
  getSharedNoteDescription,
  parseSharedNoteDocument,
  withoutDuplicateLeadingTitle,
} from "@/lib/shared-notes";

const accessibleSessionRowSchema = z.object({
  share_id: z.string().uuid(),
  manage_access: z.boolean(),
});

const galleryRowSchema = z.object({
  share_id: z.string().uuid(),
  general_scope: z.enum(["restricted", "workspace", "link", "public"]),
  title: z.string().nullable(),
  body_json: z.unknown().nullable(),
  has_snapshot: z.boolean(),
  published_at: z.string(),
});

const listManagedSharesInput = z
  .object({
    query: z.string().trim().max(200).optional(),
    afterPublishedAt: z.string().datetime({ offset: true }).optional(),
    afterShareId: z.string().uuid().optional(),
  })
  .refine(
    (value) => Boolean(value.afterPublishedAt) === Boolean(value.afterShareId),
    "invalid shared note cursor",
  );

const MANAGED_SHARES_PAGE_SIZE = 12;

export type ManagedShare = {
  shareId: string;
  title: string;
  preview: string;
  hasSnapshot: boolean;
  scope: "restricted" | "workspace" | "link" | "public";
  updatedAt: string;
};

export type ManagedSharesResult =
  | {
      status: "ready";
      shares: ManagedShare[];
      nextCursor: { publishedAt: string; shareId: string } | null;
    }
  | { status: "error" };

export const listMyManagedShares = createServerFn({ method: "GET" })
  .inputValidator(listManagedSharesInput)
  .handler(async ({ data }): Promise<ManagedSharesResult> => {
    setResponseHeader("Cache-Control", "no-store");

    const supabase = getSupabaseServerClient();
    const { data: rows, error } = await supabase.rpc(
      "list_my_managed_share_gallery_page",
      {
        p_query: data.query ?? null,
        p_after_published_at: data.afterPublishedAt ?? null,
        p_after_share_id: data.afterShareId ?? null,
        p_limit: MANAGED_SHARES_PAGE_SIZE + 1,
      },
    );
    const parsedRows = z.array(galleryRowSchema).safeParse(rows);
    if (error || !parsedRows.success) {
      return { status: "error" };
    }

    const shares = parsedRows.data
      .slice(0, MANAGED_SHARES_PAGE_SIZE)
      .map((row) => ({
        shareId: row.share_id,
        title: row.title ?? "",
        preview: getSnapshotPreview(row.body_json, row.title ?? ""),
        hasSnapshot: row.has_snapshot,
        scope: row.general_scope,
        updatedAt: row.published_at,
      }));
    const lastShare = shares.at(-1);
    const nextCursor =
      parsedRows.data.length > MANAGED_SHARES_PAGE_SIZE && lastShare
        ? { publishedAt: lastShare.updatedAt, shareId: lastShare.shareId }
        : null;

    return { status: "ready", shares, nextCursor };
  });

function getSnapshotPreview(body: unknown, title: string) {
  try {
    const document = withoutDuplicateLeadingTitle(
      parseSharedNoteDocument(body),
      title,
    );
    return getSharedNoteDescription(document);
  } catch {
    return "";
  }
}

async function listManagedShareIds() {
  const supabase = getSupabaseServerClient();
  const { data, error } = await supabase.rpc("list_my_accessible_sessions");
  if (error || !Array.isArray(data)) {
    return null;
  }

  const parsedRows = z.array(accessibleSessionRowSchema).safeParse(data);
  if (!parsedRows.success) {
    return null;
  }
  return parsedRows.data
    .filter((row) => row.manage_access)
    .map((row) => row.share_id);
}

export const deleteMyShare = createServerFn({ method: "POST" })
  .inputValidator(z.object({ shareId: z.string().uuid() }))
  .handler(async ({ data }) => {
    const supabase = getSupabaseServerClient();
    const { error } = await supabase.rpc("delete_session_share", {
      p_share_id: data.shareId,
    });

    if (error) {
      return { success: false as const, message: error.message };
    }
    return { success: true as const };
  });

export const restrictMyShare = createServerFn({ method: "POST" })
  .inputValidator(z.object({ shareId: z.string().uuid() }))
  .handler(async ({ data }) => {
    const supabase = getSupabaseServerClient();
    const { error } = await supabase.rpc("set_session_share_scope", {
      p_share_id: data.shareId,
      p_general_scope: "restricted",
    });

    if (error) {
      return { success: false as const, message: error.message };
    }
    return { success: true as const };
  });

export const deleteMyShares = createServerFn({ method: "POST" }).handler(
  async () => {
    const shareIds = await listManagedShareIds();
    if (!shareIds) {
      return {
        success: false as const,
        message: "Failed to load shared notes",
      };
    }
    if (shareIds.length === 0) {
      return { success: true as const };
    }

    const supabase = getSupabaseServerClient();
    let failed = 0;

    for (const shareId of shareIds) {
      const { error } = await supabase.rpc("delete_session_share", {
        p_share_id: shareId,
      });
      if (error) {
        failed += 1;
      }
    }

    if (failed === shareIds.length) {
      return {
        success: false as const,
        message: "Failed to stop sharing your notes",
      };
    }
    if (failed > 0) {
      return {
        success: false as const,
        message: `Couldn't stop sharing ${failed} ${
          failed === 1 ? "note" : "notes"
        }. Try again.`,
      };
    }
    return { success: true as const };
  },
);
