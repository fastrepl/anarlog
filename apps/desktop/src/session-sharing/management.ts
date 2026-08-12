import { isTauri } from "@tauri-apps/api/core";
import { writeText as writeClipboardText } from "@tauri-apps/plugin-clipboard-manager";

import {
  enableSessionShareLink,
  getSessionShareManagement,
  listSessionShareAccess,
  revokeSessionAccessInvitation,
  rotateSessionShareLink,
  setSessionShareScope,
  type SessionShareAccessEntry,
  type SessionShareManagement,
  type ShareManagementContext,
  ShareManagementError,
} from "./client";
import {
  buildAccountSessionShareUrl,
  buildPublicSessionShareUrl,
  buildSessionInvitationUrl,
  buildSessionShareLinkUrl,
  type ShareDesktopScheme,
} from "./urls";

import { useAuth } from "~/auth";
import { env } from "~/env";
import { getScheme } from "~/shared/utils";

export type SharePanelData = {
  management: SessionShareManagement;
  access: SessionShareAccessEntry[];
  wasCreated?: boolean;
};

export type SharePreparationIdentity = {
  ownerUserId: string;
  sessionId: string;
  attemptId: number;
};

export type SharePanelIdentity = SharePreparationIdentity & {
  shareId: string;
};

export class ShareOperationAbortedError extends ShareManagementError {
  constructor() {
    super();
    this.name = "ShareOperationAbortedError";
  }
}

export function sessionShareManagementQueryKey(
  ownerUserId: string,
  shareId: string,
) {
  return ["session-share-management", ownerUserId, shareId] as const;
}

export async function loadSharePanel(
  context: ShareManagementContext,
  shareId: string,
): Promise<SharePanelData> {
  const [management, access] = await Promise.all([
    getSessionShareManagement(context, shareId),
    listSessionShareAccess(context, shareId),
  ]);
  return { management, access };
}

export function requireManagementContext(
  auth: ReturnType<typeof useAuth>,
): ShareManagementContext {
  if (
    !auth.supabase ||
    !auth.session ||
    auth.session.user.is_anonymous === true
  ) {
    throw new ShareManagementError();
  }
  return { supabase: auth.supabase, session: auth.session };
}

export async function copyText(value: string) {
  if (isTauri()) {
    await writeClipboardText(value);
    return;
  }
  await navigator.clipboard.writeText(value);
}

export async function copySessionShareUrl(
  shareId: string,
  assertActive: () => unknown,
) {
  assertActive();
  const desktopScheme = await getSessionShareDesktopScheme();
  assertActive();
  await copyText(
    buildAccountSessionShareUrl({
      appBaseUrl: env.VITE_APP_URL,
      shareId,
      desktopScheme,
    }),
  );
  assertActive();
}

export async function enableAndCopySessionShareLink({
  context,
  shareId,
  hasActiveLink,
  assertActive,
}: {
  context: ShareManagementContext;
  shareId: string;
  hasActiveLink: boolean;
  assertActive: () => unknown;
}) {
  try {
    let link = hasActiveLink
      ? await rotateSessionShareLink(context, shareId)
      : await enableSessionShareLink(context, shareId);
    if (!link.linkToken) {
      link = await rotateSessionShareLink(context, shareId);
    }
    if (!link.linkToken) throw new ShareManagementError();
    assertActive();
    const desktopScheme = await getSessionShareDesktopScheme();
    assertActive();
    await copyText(
      buildSessionShareLinkUrl({
        appBaseUrl: env.VITE_APP_URL,
        linkId: link.linkId,
        linkToken: link.linkToken,
        desktopScheme,
      }),
    );
    assertActive();
  } catch {
    await setSessionShareScope(withoutSignal(context), {
      shareId,
      scope: "restricted",
    }).catch(() => undefined);
    throw new ShareManagementError();
  }
}

export async function copyPublicSessionShareUrl(
  publicSlug: string,
  assertActive: () => unknown,
) {
  assertActive();
  const desktopScheme = await getSessionShareDesktopScheme();
  assertActive();
  await copyText(
    buildPublicSessionShareUrl({
      appBaseUrl: env.VITE_APP_URL,
      publicSlug,
      desktopScheme,
    }),
  );
  assertActive();
}

export async function copyInvitationOrRevoke(
  context: ShareManagementContext,
  invitation: { invitationId: string; inviteToken: string },
  assertActive: () => unknown,
  signal?: AbortSignal,
) {
  try {
    assertActive();
    await copyText(
      buildSessionInvitationUrl({
        appBaseUrl: env.VITE_APP_URL,
        invitationId: invitation.invitationId,
        inviteToken: invitation.inviteToken,
        desktopScheme: await getSessionShareDesktopScheme(),
      }),
    );
    assertActive();
  } catch {
    if (signal?.aborted) throw new ShareOperationAbortedError();
    await revokeSessionAccessInvitation(context, invitation.invitationId).catch(
      () => undefined,
    );
    throw new ShareManagementError();
  }
}

export function withoutSignal(
  context: ShareManagementContext,
): ShareManagementContext {
  return { supabase: context.supabase, session: context.session };
}

export async function getSessionShareDesktopScheme(): Promise<ShareDesktopScheme> {
  return getScheme();
}
