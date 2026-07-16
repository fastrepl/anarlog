import type { Session, SupabaseClient } from "@supabase/supabase-js";
import { useQuery } from "@tanstack/react-query";

import {
  parseDurableSharedNoteSnapshots,
  replaceDurableSharedNoteCache,
} from "./cache";

import { useAuth } from "~/auth";
import { useTabs } from "~/store/zustand/tabs";

const REFRESH_INTERVAL_MS = 60 * 1000;
const PAGE_SIZE = 100;
const MAX_AGGREGATE_BYTES = 64 * 1024 * 1024;

export async function fetchDurableSharedNoteSnapshots(
  supabase: SupabaseClient,
  session: Session,
  signal: AbortSignal,
) {
  const rows: unknown[] = [];
  let aggregateBytes = 0;
  let afterShareId: string | null = null;

  for (;;) {
    signal.throwIfAborted();
    const response: { data: unknown; error: unknown } = await supabase
      .rpc("list_my_session_share_snapshot_page", {
        p_after_share_id: afterShareId,
        p_limit: PAGE_SIZE,
      })
      .setHeader(
        "Authorization",
        `${session.token_type} ${session.access_token}`,
      )
      .abortSignal(signal);
    const { data, error } = response;
    if (error) {
      throw error;
    }
    if (!Array.isArray(data) || data.length > PAGE_SIZE) {
      throw new Error("invalid durable shared-note snapshot page");
    }

    aggregateBytes += new TextEncoder().encode(JSON.stringify(data)).byteLength;
    if (aggregateBytes > MAX_AGGREGATE_BYTES) {
      throw new Error("durable shared-note snapshot list is too large");
    }
    rows.push(...data);

    if (data.length < PAGE_SIZE) {
      break;
    }

    const cursor: unknown = data[data.length - 1];
    if (
      !cursor ||
      typeof cursor !== "object" ||
      !("share_id" in cursor) ||
      typeof cursor.share_id !== "string" ||
      (afterShareId !== null && cursor.share_id <= afterShareId)
    ) {
      throw new Error("invalid durable shared-note snapshot cursor");
    }
    afterShareId = cursor.share_id;
  }

  return parseDurableSharedNoteSnapshots(rows);
}

export async function syncDurableSharedNoteCache(
  supabase: SupabaseClient,
  session: Session,
  signal: AbortSignal,
) {
  const snapshots = await fetchDurableSharedNoteSnapshots(
    supabase,
    session,
    signal,
  );
  signal.throwIfAborted();
  await replaceDurableSharedNoteCache(session.user.id, snapshots);
  signal.throwIfAborted();

  const authorizedShareIds = new Set(
    snapshots.map((snapshot) => snapshot.shareId),
  );
  const tabs = useTabs.getState();
  const knownSharedTabIds = new Set<string>();
  for (const tab of tabs.tabs) {
    if (tab.type === "shared_sessions") {
      knownSharedTabIds.add(tab.id);
    }
  }
  for (const history of tabs.history.values()) {
    for (const tab of history.stack) {
      if (tab.type === "shared_sessions") {
        knownSharedTabIds.add(tab.id);
      }
    }
  }
  for (const shareId of knownSharedTabIds) {
    if (!authorizedShareIds.has(shareId)) {
      tabs.invalidateResource("shared_sessions", shareId);
    }
  }

  return {
    count: snapshots.length,
    accessVersion: Math.max(
      0,
      ...snapshots.map((snapshot) => snapshot.accessVersion),
    ),
  };
}

export function DurableSharedNoteCacheSync() {
  const { session, supabase } = useAuth();
  const viewerUserId = session?.user.id ?? null;
  const isPermanentUser = session?.user.is_anonymous !== true;

  useQuery({
    queryKey: ["durable-shared-note-cache", viewerUserId, session?.expires_at],
    enabled: Boolean(supabase && session && viewerUserId && isPermanentUser),
    queryFn: ({ signal }) =>
      syncDurableSharedNoteCache(supabase!, session!, signal),
    refetchInterval: REFRESH_INTERVAL_MS,
    refetchIntervalInBackground: false,
  });

  return null;
}
