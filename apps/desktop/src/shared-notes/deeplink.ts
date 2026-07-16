import type { ShareOpenRequest } from "@hypr/plugin-deeplink2";

import {
  parseDurableSharedNoteSnapshots,
  upsertDurableSharedNoteCache,
} from "./cache";
import {
  beginSharedNotePreview,
  claimSharedNoteHandoff,
  purgeSharedNotePreview,
} from "./preview";

import type { AuthContextType } from "~/auth/auth-context";
import type { TabInput } from "~/store/zustand/tabs";

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const MAX_SEEN_PENDING_IDS = 128;

type CommandResult<T> =
  | { status: "ok"; data: T }
  | { status: "error"; error: string };

export async function openAccountSharedNote({
  shareId,
  auth,
  openNew,
  signal,
}: {
  shareId: string;
  auth: AuthContextType;
  openNew: (tab: TabInput) => void;
  signal: AbortSignal;
}) {
  const { session, supabase } = auth;
  if (
    !session ||
    session.user.is_anonymous === true ||
    !supabase ||
    signal.aborted
  ) {
    openNew({ type: "shared_sessions", id: shareId });
    return;
  }

  try {
    const response: { data: unknown; error: unknown } = await supabase
      .rpc("read_my_session_share_snapshot", { p_share_id: shareId })
      .setHeader(
        "Authorization",
        `${session.token_type} ${session.access_token}`,
      )
      .abortSignal(signal);
    if (response.error) {
      throw new Error("shared-note access unavailable");
    }
    const snapshots = parseDurableSharedNoteSnapshots(response.data);
    if (snapshots.length !== 1 || snapshots[0]?.shareId !== shareId) {
      throw new Error("shared-note access unavailable");
    }
    signal.throwIfAborted();
    await upsertDurableSharedNoteCache(session.user.id, snapshots[0]);
    signal.throwIfAborted();
  } catch {
    if (signal.aborted) {
      return;
    }
  }

  openNew({ type: "shared_sessions", id: shareId });
}

export function openHandoffSharedNote({
  requestId,
  openNew,
  claim = claimSharedNoteHandoff,
  createViewId,
}: {
  requestId: string;
  openNew: (tab: TabInput) => void;
  claim?: typeof claimSharedNoteHandoff;
  createViewId?: () => string;
}) {
  const viewId = beginSharedNotePreview(
    (signal) => claim(requestId, signal),
    createViewId,
  );
  try {
    openNew({ type: "shared_note_preview", id: viewId });
  } catch {
    purgeSharedNotePreview(viewId);
    throw new Error("shared-note preview unavailable");
  }
}

export function createShareOpenProcessor({
  takePendingShareOpen,
  getAuth,
  openNew,
  openAccount = openAccountSharedNote,
  openHandoff = openHandoffSharedNote,
}: {
  takePendingShareOpen: (
    pendingId: string,
  ) => Promise<CommandResult<ShareOpenRequest | null>>;
  getAuth: () => AuthContextType;
  openNew: (tab: TabInput) => void;
  openAccount?: typeof openAccountSharedNote;
  openHandoff?: typeof openHandoffSharedNote;
}) {
  const seen = new Set<string>();
  const accountControllers = new Set<AbortController>();
  let disposed = false;

  const handle = async (pendingId: string) => {
    if (disposed || !UUID_PATTERN.test(pendingId) || seen.has(pendingId)) {
      return;
    }
    rememberPendingId(seen, pendingId);

    let result: CommandResult<ShareOpenRequest | null>;
    try {
      result = await takePendingShareOpen(pendingId);
    } catch {
      return;
    }
    if (disposed || result.status === "error" || !result.data) {
      return;
    }

    if (result.data.mode === "account") {
      const controller = new AbortController();
      accountControllers.add(controller);
      await openAccount({
        shareId: result.data.share_id,
        auth: getAuth(),
        openNew,
        signal: controller.signal,
      });
      accountControllers.delete(controller);
      return;
    }

    try {
      openHandoff({
        requestId: result.data.request_id,
        openNew,
      });
    } catch {
      return;
    }
  };

  return {
    handle,
    dispose() {
      disposed = true;
      for (const controller of accountControllers) {
        controller.abort();
      }
      accountControllers.clear();
      seen.clear();
    },
  };
}

export async function subscribeThenDrainShareOpens({
  listen,
  listPendingShareOpens,
  handle,
}: {
  listen: (handler: (pendingId: string) => void) => Promise<() => void>;
  listPendingShareOpens: () => Promise<CommandResult<string[]>>;
  handle: (pendingId: string) => Promise<void>;
}) {
  const unlisten = await listen((pendingId) => {
    void handle(pendingId);
  });

  let result: CommandResult<string[]>;
  try {
    result = await listPendingShareOpens();
  } catch {
    return unlisten;
  }
  if (result.status === "ok") {
    await Promise.all(result.data.map((pendingId) => handle(pendingId)));
  }
  return unlisten;
}

function rememberPendingId(seen: Set<string>, pendingId: string) {
  if (seen.size >= MAX_SEEN_PENDING_IDS) {
    const oldest = seen.values().next().value;
    if (oldest) {
      seen.delete(oldest);
    }
  }
  seen.add(pendingId);
}
