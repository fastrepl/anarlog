import { createServerFn } from "@tanstack/react-start";
import { setResponseHeader } from "@tanstack/react-start/server";
import { z } from "zod";

import {
  getSupabaseAdminClient,
  getSupabaseServerClient,
} from "@/functions/supabase";
import {
  getSharedNoteDescription,
  parseSharedNoteDocument,
  withoutDuplicateLeadingTitle,
} from "@/lib/shared-notes";

const accessibleSessionRowSchema = z.object({
  share_id: z.string().uuid(),
  manage_access: z.boolean(),
});

const shareDetailRowSchema = z.object({
  id: z.string().uuid(),
  general_scope: z.enum(["restricted", "workspace", "link", "public"]),
  updated_at: z.string(),
});

const snapshotRowSchema = z.object({
  share_id: z.string().uuid(),
  title: z.string(),
  body_json: z.unknown(),
});

const shareIdRowSchema = z.object({ share_id: z.string().uuid() });
const listManagedSharesInput = z
  .object({
    query: z.string().trim().max(200).optional(),
    afterUpdatedAt: z.string().datetime({ offset: true }).optional(),
    afterShareId: z.string().uuid().optional(),
  })
  .refine(
    (value) => Boolean(value.afterUpdatedAt) === Boolean(value.afterShareId),
    "invalid shared note cursor",
  );

const MANAGED_SHARES_PAGE_SIZE = 12;

export type ManagedShare = {
  shareId: string;
  title: string;
  preview: string;
  scope: "restricted" | "workspace" | "link" | "public";
  updatedAt: string;
};

export type ManagedSharesResult =
  | {
      status: "ready";
      shares: ManagedShare[];
      nextCursor: { updatedAt: string; shareId: string } | null;
    }
  | { status: "error" };

export const listMyManagedShares = createServerFn({ method: "GET" })
  .inputValidator(listManagedSharesInput)
  .handler(async ({ data }): Promise<ManagedSharesResult> => {
    setResponseHeader("Cache-Control", "no-store");

    const managedIds = await listManagedShareIds();
    if (!managedIds) {
      return { status: "error" };
    }
    if (managedIds.length === 0) {
      return { status: "ready", shares: [], nextCursor: null };
    }

    const admin = getSupabaseAdminClient();
    let candidateIds = managedIds;
    if (data.query) {
      const matchesRes = await admin
        .from("session_share_snapshots")
        .select("share_id")
        .in("share_id", managedIds)
        .ilike("title", `%${escapeLikePattern(data.query)}%`);
      const parsedMatches = z
        .array(shareIdRowSchema)
        .safeParse(matchesRes.data);
      if (matchesRes.error || !parsedMatches.success) {
        return { status: "error" };
      }
      candidateIds = parsedMatches.data.map((row) => row.share_id);
    }
    if (candidateIds.length === 0) {
      return { status: "ready", shares: [], nextCursor: null };
    }

    let sharesQuery = admin
      .from("session_shares")
      .select("id, general_scope, updated_at")
      .in("id", candidateIds)
      .is("deleted_at", null);
    if (data.afterUpdatedAt && data.afterShareId) {
      sharesQuery = sharesQuery.or(
        `updated_at.lt.${data.afterUpdatedAt},and(updated_at.eq.${data.afterUpdatedAt},id.lt.${data.afterShareId})`,
      );
    }
    const sharesRes = await sharesQuery
      .order("updated_at", { ascending: false })
      .order("id", { ascending: false })
      .limit(MANAGED_SHARES_PAGE_SIZE + 1);
    const parsedShares = z
      .array(shareDetailRowSchema)
      .safeParse(sharesRes.data);
    if (sharesRes.error || !parsedShares.success) {
      return { status: "error" };
    }

    const pageRows = parsedShares.data.slice(0, MANAGED_SHARES_PAGE_SIZE);
    if (pageRows.length === 0) {
      return { status: "ready", shares: [], nextCursor: null };
    }
    const pageIds = pageRows.map((row) => row.id);
    const snapshotsRes = await admin
      .from("session_share_snapshots")
      .select("share_id, title, body_json")
      .in("share_id", pageIds);
    const parsedSnapshots = z
      .array(snapshotRowSchema)
      .safeParse(snapshotsRes.data);
    if (snapshotsRes.error || !parsedSnapshots.success) {
      return { status: "error" };
    }

    const snapshots = new Map(
      parsedSnapshots.data.map((row) => [
        row.share_id,
        {
          title: row.title,
          preview: getSnapshotPreview(row.body_json, row.title),
        },
      ]),
    );

    const shares = pageRows.map((row) => ({
      shareId: row.id,
      title: snapshots.get(row.id)?.title ?? "",
      preview: snapshots.get(row.id)?.preview ?? "",
      scope: row.general_scope,
      updatedAt: row.updated_at,
    }));
    const lastShare = shares.at(-1);
    const nextCursor =
      parsedShares.data.length > MANAGED_SHARES_PAGE_SIZE && lastShare
        ? { updatedAt: lastShare.updatedAt, shareId: lastShare.shareId }
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

function escapeLikePattern(value: string) {
  return value.replace(/[\\%_]/g, "\\$&");
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
