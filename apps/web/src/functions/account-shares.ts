import { createServerFn } from "@tanstack/react-start";
import { setResponseHeader } from "@tanstack/react-start/server";
import { z } from "zod";

import {
  getSupabaseAdminClient,
  getSupabaseServerClient,
} from "@/functions/supabase";

const accessibleSessionRowSchema = z.object({
  share_id: z.string().uuid(),
  manage_access: z.boolean(),
});

const shareDetailRowSchema = z.object({
  id: z.string().uuid(),
  general_scope: z.enum(["restricted", "workspace", "link", "public"]),
  created_at: z.string(),
  updated_at: z.string(),
});

const snapshotRowSchema = z.object({
  share_id: z.string().uuid(),
  title: z.string(),
});

export type ManagedShare = {
  shareId: string;
  title: string;
  scope: "restricted" | "workspace" | "link" | "public";
  updatedAt: string;
};

export type ManagedSharesResult =
  | { status: "ready"; shares: ManagedShare[] }
  | { status: "error" };

export const listMyManagedShares = createServerFn({ method: "GET" }).handler(
  async (): Promise<ManagedSharesResult> => {
    setResponseHeader("Cache-Control", "no-store");

    const supabase = getSupabaseServerClient();
    const { data, error } = await supabase.rpc("list_my_accessible_sessions");
    if (error || !Array.isArray(data)) {
      return { status: "error" };
    }

    const parsedRows = z.array(accessibleSessionRowSchema).safeParse(data);
    if (!parsedRows.success) {
      return { status: "error" };
    }

    const managedIds = parsedRows.data
      .filter((row) => row.manage_access)
      .map((row) => row.share_id);
    if (managedIds.length === 0) {
      return { status: "ready", shares: [] };
    }

    const admin = getSupabaseAdminClient();
    const [sharesRes, snapshotsRes] = await Promise.all([
      admin
        .from("session_shares")
        .select("id, general_scope, created_at, updated_at")
        .in("id", managedIds)
        .is("deleted_at", null),
      admin
        .from("session_share_snapshots")
        .select("share_id, title")
        .in("share_id", managedIds),
    ]);
    if (sharesRes.error || snapshotsRes.error) {
      return { status: "error" };
    }

    const parsedShares = z
      .array(shareDetailRowSchema)
      .safeParse(sharesRes.data);
    const parsedSnapshots = z
      .array(snapshotRowSchema)
      .safeParse(snapshotsRes.data);
    if (!parsedShares.success || !parsedSnapshots.success) {
      return { status: "error" };
    }

    const titles = new Map(
      parsedSnapshots.data.map((row) => [row.share_id, row.title]),
    );

    const shares = parsedShares.data
      .map((row) => ({
        shareId: row.id,
        title: titles.get(row.id) ?? "",
        scope: row.general_scope,
        updatedAt: row.updated_at,
      }))
      .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));

    return { status: "ready", shares };
  },
);

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

export const deleteMyShares = createServerFn({ method: "POST" })
  .inputValidator(
    z.object({ shareIds: z.array(z.string().uuid()).min(1).max(100) }),
  )
  .handler(async ({ data }) => {
    const supabase = getSupabaseServerClient();
    let failed = 0;

    for (const shareId of data.shareIds) {
      const { error } = await supabase.rpc("delete_session_share", {
        p_share_id: shareId,
      });
      if (error) {
        failed += 1;
      }
    }

    if (failed === data.shareIds.length) {
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
  });
