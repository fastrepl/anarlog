import { createServerFn } from "@tanstack/react-start";
import { setResponseHeader } from "@tanstack/react-start/server";
import { z } from "zod";

import { env } from "@/env";
import { getSupabaseServerClient } from "@/functions/supabase";
import {
  fetchPublicSharedNoteResult,
  type SharedNoteReadResult,
} from "@/lib/shared-note-api";
import {
  MAX_SHARED_NOTE_COMMENT_BYTES,
  validateSharedNoteCommentBody,
} from "@/lib/shared-note-collaboration";
import { deriveSharedNoteEditorTitle } from "@/lib/shared-note-editing";
import {
  type AuthenticatedSharedNote,
  handoffRequestIdSchema,
  parseSessionAccessRequestState,
  parseSessionInvitationState,
  parseSessionShareAccessPage,
  parseAuthenticatedSharedNote,
  parseSharedNoteDocument,
  parseSharedNoteWebEditConflict,
  parseSharedNoteWebEditSnapshot,
  parseSharedNoteComment,
  parseSharedNoteCommentPage,
  parseSharedNoteAttachmentDownload,
  publicShareSlugSchema,
  type SessionAccessRequestState,
  type SessionInvitationState,
  type SessionShareAccessPage,
  shareIdSchema,
  type SharedNoteComment,
  type SharedNoteCommentPage,
  type SharedNoteWebEditSnapshot,
} from "@/lib/shared-notes";

export type AuthenticatedSharedNoteReadResult =
  | { status: "ready"; note: AuthenticatedSharedNote }
  | { status: "unavailable" }
  | { status: "error" };

export type SharedNoteWebEditResult =
  | ({ status: "ready" } & SharedNoteWebEditSnapshot)
  | ({ status: "conflict" } & SharedNoteWebEditSnapshot)
  | { status: "sign_in_required" }
  | { status: "unavailable" }
  | { status: "error" };

const attachmentDownloadInputSchema = z
  .object({
    shareId: shareIdSchema,
    attachmentId: shareIdSchema,
  })
  .strict();

const sharedNoteWebEditInputSchema = z
  .object({
    shareId: shareIdSchema,
    baseRevision: z.number().int().positive().safe(),
    mutationId: handoffRequestIdSchema,
    title: z.string().trim().max(4096),
    body: z.unknown(),
    attachmentIds: z.array(handoffRequestIdSchema).max(64),
  })
  .strict()
  .refine(
    ({ attachmentIds }) => new Set(attachmentIds).size === attachmentIds.length,
    { path: ["attachmentIds"] },
  );

const MAX_WEB_EDIT_RESPONSE_BYTES = 2_250_000;

const sharedNoteCommentInputSchema = z
  .object({
    shareId: shareIdSchema,
    body: z.string().max(MAX_SHARED_NOTE_COMMENT_BYTES),
  })
  .strict();

const listSharedNoteCommentsInputSchema = z
  .object({
    shareId: shareIdSchema,
    beforeCreatedAt: z.iso.datetime({ offset: true }).max(64).nullable(),
    beforeCommentId: shareIdSchema.nullable(),
  })
  .strict()
  .refine(
    ({ beforeCreatedAt, beforeCommentId }) =>
      (beforeCreatedAt === null) === (beforeCommentId === null),
  );

const reviewSharedNoteAccessRequestInputSchema = z.discriminatedUnion(
  "decision",
  [
    z
      .object({
        requestId: shareIdSchema,
        decision: z.literal("approved"),
        capability: z.enum(["viewer", "commenter", "editor"]),
      })
      .strict(),
    z
      .object({
        requestId: shareIdSchema,
        decision: z.literal("denied"),
      })
      .strict(),
  ],
);

const listSharedNoteManagerAccessInputSchema = z
  .object({
    shareId: shareIdSchema,
    beforeCreatedAt: z.iso.datetime({ offset: true }).max(64).nullable(),
    beforeEntryId: shareIdSchema.nullable(),
  })
  .strict()
  .refine(
    ({ beforeCreatedAt, beforeEntryId }) =>
      (beforeCreatedAt === null) === (beforeEntryId === null),
  );

const invitationActionInputSchema = z
  .object({
    invitationId: shareIdSchema,
    token: z.string().regex(/^[A-Za-z0-9_-]{43}$/),
  })
  .strict();

const requestSessionAccessRowSchema = z
  .object({
    request_id: shareIdSchema,
    requested_capability: z.literal("commenter"),
    was_created: z.boolean(),
  })
  .strict();

const deletedCommentRowSchema = z
  .object({
    comment_id: shareIdSchema,
    deleted_at: z.iso.datetime({ offset: true }).max(64),
  })
  .strict();

const cancelledAccessRequestRowSchema = z
  .object({
    request_id: shareIdSchema,
    status: z.literal("cancelled"),
  })
  .strict();

const reviewedAccessRequestRowSchema = z.discriminatedUnion("status", [
  z
    .object({
      request_id: shareIdSchema,
      status: z.literal("approved"),
      grant_id: shareIdSchema,
      capability: z.enum(["viewer", "commenter", "editor"]),
    })
    .strict(),
  z
    .object({
      request_id: shareIdSchema,
      status: z.literal("denied"),
      grant_id: z.null(),
      capability: z.null(),
    })
    .strict(),
]);

const acceptedInvitationRowSchema = z
  .object({
    share_id: shareIdSchema,
    grant_id: shareIdSchema,
    capability: z.enum(["viewer", "commenter", "editor"]),
  })
  .strict();

type SharedNoteOperationStatus =
  | { status: "ready" }
  | { status: "unavailable" }
  | { status: "error" };

type SharedNoteAccessRequestResult =
  | { status: "ready"; request: SessionAccessRequestState | null }
  | { status: "error" };

export const readAuthenticatedSharedNote = createServerFn({ method: "GET" })
  .inputValidator(shareIdSchema)
  .handler(
    async ({ data: shareId }): Promise<AuthenticatedSharedNoteReadResult> => {
      setPrivateShareResponseHeaders();

      const supabase = getSupabaseServerClient();
      const { data, error } = await supabase.rpc(
        "read_my_session_share_snapshot_v2",
        { p_share_id: shareId },
      );
      if (error || !Array.isArray(data)) {
        return { status: "error" };
      }
      if (data.length === 0) {
        return { status: "unavailable" };
      }
      if (data.length !== 1) {
        return { status: "error" };
      }

      try {
        return { status: "ready", note: parseAuthenticatedSharedNote(data[0]) };
      } catch {
        return { status: "error" };
      }
    },
  );

export const readPublicSharedNote = createServerFn({ method: "GET" })
  .inputValidator(publicShareSlugSchema)
  .handler(async ({ data: publicSlug }): Promise<SharedNoteReadResult> => {
    setResponseHeader("Cache-Control", "no-store");
    setResponseHeader("Referrer-Policy", "no-referrer");
    return fetchPublicSharedNoteResult(publicSlug);
  });

export const listSharedNoteComments = createServerFn({ method: "GET" })
  .inputValidator(listSharedNoteCommentsInputSchema)
  .handler(
    async ({
      data: input,
    }): Promise<
      | ({ status: "ready" } & SharedNoteCommentPage)
      | { status: "unavailable" }
      | { status: "error" }
    > => {
      setPrivateShareResponseHeaders();

      const supabase = getSupabaseServerClient();
      const { data, error } = await supabase.rpc(
        "list_session_share_comments",
        {
          p_share_id: input.shareId,
          p_before_created_at: input.beforeCreatedAt,
          p_before_comment_id: input.beforeCommentId,
          p_limit: 101,
        },
      );
      if (error) return unavailableOrError(error.code);

      try {
        return {
          status: "ready",
          ...parseSharedNoteCommentPage(data),
        };
      } catch {
        return { status: "error" };
      }
    },
  );

export const createSharedNoteComment = createServerFn({ method: "POST" })
  .inputValidator(sharedNoteCommentInputSchema)
  .handler(
    async ({
      data,
    }): Promise<
      | { status: "ready"; comment: SharedNoteComment }
      | { status: "unavailable" }
      | { status: "error" }
    > => {
      setPrivateShareResponseHeaders();
      const comment = validateSharedNoteCommentBody(data.body);
      if (!comment.valid) {
        return { status: "unavailable" };
      }

      const supabase = getSupabaseServerClient();
      const { data: commentRows, error } = await supabase.rpc(
        "create_session_share_comment",
        {
          p_share_id: data.shareId,
          p_body: comment.body,
        },
      );
      if (error) return unavailableOrError(error.code);
      if (!Array.isArray(commentRows) || commentRows.length === 0) {
        return { status: "unavailable" };
      }
      if (commentRows.length !== 1) return { status: "error" };

      try {
        return {
          status: "ready",
          comment: parseSharedNoteComment(commentRows[0]),
        };
      } catch {
        return { status: "error" };
      }
    },
  );

export const deleteSharedNoteComment = createServerFn({ method: "POST" })
  .inputValidator(shareIdSchema)
  .handler(async ({ data: commentId }): Promise<SharedNoteOperationStatus> => {
    setPrivateShareResponseHeaders();

    const supabase = getSupabaseServerClient();
    const { data, error } = await supabase.rpc("delete_session_share_comment", {
      p_comment_id: commentId,
    });
    if (error) return unavailableOrError(error.code);
    if (!Array.isArray(data) || data.length === 0) {
      return { status: "unavailable" };
    }
    if (data.length !== 1) return { status: "error" };

    try {
      const parsed = deletedCommentRowSchema.parse(data[0]);
      return parsed.comment_id === commentId
        ? { status: "ready" }
        : { status: "error" };
    } catch {
      return { status: "error" };
    }
  });

export const getMySharedNoteAccessRequest = createServerFn({ method: "GET" })
  .inputValidator(shareIdSchema)
  .handler(
    async ({ data: shareId }): Promise<SharedNoteAccessRequestResult> => {
      setPrivateShareResponseHeaders();
      return loadMySharedNoteAccessRequest(shareId);
    },
  );

export const requestSharedNoteCommentAccess = createServerFn({ method: "POST" })
  .inputValidator(shareIdSchema)
  .handler(
    async ({ data: shareId }): Promise<SharedNoteAccessRequestResult> => {
      setPrivateShareResponseHeaders();

      const supabase = getSupabaseServerClient();
      const { data, error } = await supabase.rpc("request_session_access", {
        p_share_id: shareId,
        p_requested_capability: "commenter",
      });
      if (error || !Array.isArray(data) || data.length !== 1) {
        return { status: "error" };
      }

      try {
        const requested = requestSessionAccessRowSchema.parse(data[0]);
        const result = await loadMySharedNoteAccessRequest(shareId);
        if (
          result.status !== "ready" ||
          result.request?.requestId !== requested.request_id ||
          result.request.requestedCapability !== "commenter" ||
          result.request.status !== "pending"
        ) {
          return { status: "error" };
        }
        return result;
      } catch {
        return { status: "error" };
      }
    },
  );

export const cancelMySharedNoteAccessRequest = createServerFn({
  method: "POST",
})
  .inputValidator(shareIdSchema)
  .handler(async ({ data: requestId }): Promise<SharedNoteOperationStatus> => {
    setPrivateShareResponseHeaders();

    const supabase = getSupabaseServerClient();
    const { data, error } = await supabase.rpc(
      "cancel_session_access_request",
      { p_request_id: requestId },
    );
    if (error) return unavailableOrError(error.code);
    if (!Array.isArray(data) || data.length === 0) {
      return { status: "unavailable" };
    }
    if (data.length !== 1) return { status: "error" };

    try {
      const parsed = cancelledAccessRequestRowSchema.parse(data[0]);
      return parsed.request_id === requestId
        ? { status: "ready" }
        : { status: "error" };
    } catch {
      return { status: "error" };
    }
  });

export const listSharedNoteManagerAccess = createServerFn({ method: "GET" })
  .inputValidator(listSharedNoteManagerAccessInputSchema)
  .handler(
    async ({
      data: input,
    }): Promise<
      | ({ status: "ready" } & SessionShareAccessPage)
      | { status: "unavailable" }
      | { status: "error" }
    > => {
      setPrivateShareResponseHeaders();

      const supabase = getSupabaseServerClient();
      const { data, error } = await supabase.rpc(
        "list_session_share_access_page",
        {
          p_share_id: input.shareId,
          p_before_created_at: input.beforeCreatedAt,
          p_before_entry_id: input.beforeEntryId,
          p_limit: 101,
        },
      );
      if (error) return unavailableOrError(error.code);

      try {
        return { status: "ready", ...parseSessionShareAccessPage(data) };
      } catch {
        return { status: "error" };
      }
    },
  );

export const reviewSharedNoteAccessRequest = createServerFn({ method: "POST" })
  .inputValidator(reviewSharedNoteAccessRequestInputSchema)
  .handler(async ({ data: input }): Promise<SharedNoteOperationStatus> => {
    setPrivateShareResponseHeaders();

    const supabase = getSupabaseServerClient();
    const { data, error } = await supabase.rpc(
      "review_session_access_request",
      {
        p_request_id: input.requestId,
        p_decision: input.decision,
        p_capability: input.decision === "approved" ? input.capability : null,
      },
    );
    if (error) return unavailableOrError(error.code);
    if (!Array.isArray(data) || data.length === 0) {
      return { status: "unavailable" };
    }
    if (data.length !== 1) return { status: "error" };

    try {
      const parsed = reviewedAccessRequestRowSchema.parse(data[0]);
      return parsed.request_id === input.requestId &&
        parsed.status === input.decision
        ? { status: "ready" }
        : { status: "error" };
    } catch {
      return { status: "error" };
    }
  });

export const inspectMySharedNoteInvitation = createServerFn({ method: "POST" })
  .inputValidator(invitationActionInputSchema)
  .handler(
    async ({
      data: input,
    }): Promise<
      | { status: "ready"; invitation: SessionInvitationState }
      | { status: "unavailable" }
      | { status: "error" }
    > => {
      setPrivateShareResponseHeaders();

      const supabase = getSupabaseServerClient();
      const { data, error } = await supabase.rpc(
        "inspect_my_session_access_invitation",
        {
          p_invitation_id: input.invitationId,
          p_invite_token: input.token,
        },
      );
      if (error) return unavailableOrError(error.code);
      if (!Array.isArray(data) || data.length === 0) {
        return { status: "unavailable" };
      }
      if (data.length !== 1) return { status: "error" };

      try {
        return {
          status: "ready",
          invitation: parseSessionInvitationState(data[0]),
        };
      } catch {
        return { status: "error" };
      }
    },
  );

export const acceptSharedNoteInvitation = createServerFn({ method: "POST" })
  .inputValidator(invitationActionInputSchema)
  .handler(
    async ({
      data: input,
    }): Promise<
      | { status: "ready"; shareId: string }
      | { status: "unavailable" }
      | { status: "error" }
    > => {
      setPrivateShareResponseHeaders();

      const supabase = getSupabaseServerClient();
      const { data, error } = await supabase.rpc(
        "accept_session_access_invitation",
        {
          p_invitation_id: input.invitationId,
          p_invite_token: input.token,
        },
      );
      if (error) return unavailableOrError(error.code);
      if (!Array.isArray(data) || data.length === 0) {
        return { status: "unavailable" };
      }
      if (data.length !== 1) return { status: "error" };

      try {
        const accepted = acceptedInvitationRowSchema.parse(data[0]);
        return { status: "ready", shareId: accepted.share_id };
      } catch {
        return { status: "error" };
      }
    },
  );

export const createAuthenticatedSharedAttachmentDownload = createServerFn({
  method: "POST",
})
  .inputValidator(attachmentDownloadInputSchema)
  .handler(async ({ data }) => {
    setPrivateShareResponseHeaders();
    const supabase = getSupabaseServerClient();
    const { data: sessionData } = await supabase.auth.getSession();
    const accessToken = sessionData.session?.access_token;
    if (!accessToken) return null;

    try {
      const response = await fetch(
        new URL(
          `/shared-notes/access/${encodeURIComponent(data.shareId)}/attachments/${encodeURIComponent(data.attachmentId)}/download`,
          apiBaseUrl(),
        ),
        {
          method: "POST",
          cache: "no-store",
          headers: {
            Accept: "application/json",
            Authorization: `Bearer ${accessToken}`,
          },
        },
      );
      if (!response.ok) return null;
      const text = await response.text();
      if (new TextEncoder().encode(text).byteLength > 32 * 1024) return null;
      return parseSharedNoteAttachmentDownload(JSON.parse(text) as unknown);
    } catch {
      return null;
    }
  });

export const editAuthenticatedSharedNote = createServerFn({ method: "POST" })
  .inputValidator(sharedNoteWebEditInputSchema)
  .handler(async ({ data: input }): Promise<SharedNoteWebEditResult> => {
    setPrivateShareResponseHeaders();

    let body;
    try {
      body = parseSharedNoteDocument(input.body);
    } catch {
      return { status: "error" };
    }

    const first = body.content?.[0];
    if (
      new TextEncoder().encode(input.title).byteLength > 4096 ||
      first?.type !== "heading" ||
      (first.attrs?.level ?? 1) !== 1 ||
      deriveSharedNoteEditorTitle(body) !== input.title
    ) {
      return { status: "error" };
    }

    const supabase = getSupabaseServerClient();
    const { data: sessionData } = await supabase.auth.getSession();
    const accessToken = sessionData.session?.access_token;
    if (!accessToken) return { status: "sign_in_required" };

    try {
      const response = await fetch(
        new URL(
          `/sync/shares/${encodeURIComponent(input.shareId)}/web-edit`,
          apiBaseUrl(),
        ),
        {
          method: "PUT",
          body: JSON.stringify({
            baseRevision: input.baseRevision,
            mutationId: input.mutationId,
            title: input.title,
            body,
            attachmentIds: input.attachmentIds,
          }),
          cache: "no-store",
          credentials: "omit",
          headers: {
            Accept: "application/json",
            Authorization: `Bearer ${accessToken}`,
            "Content-Type": "application/json",
          },
          redirect: "error",
          referrerPolicy: "no-referrer",
        },
      );

      if (response.status === 401) {
        return { status: "sign_in_required" };
      }
      if (response.status === 403 || response.status === 404) {
        return { status: "unavailable" };
      }
      if (
        !response.headers
          .get("content-type")
          ?.toLowerCase()
          .startsWith("application/json")
      ) {
        return { status: "error" };
      }

      const responseBody = await readBoundedJson(
        response,
        MAX_WEB_EDIT_RESPONSE_BYTES,
      );
      if (responseBody === null) return { status: "error" };

      if (response.status === 409) {
        const edited = parseSharedNoteWebEditConflict(responseBody);
        return edited.snapshot.shareId === input.shareId &&
          edited.snapshot.contentRevision > input.baseRevision
          ? { status: "conflict", ...edited }
          : { status: "error" };
      }
      if (!response.ok) return { status: "error" };

      const edited = parseSharedNoteWebEditSnapshot(responseBody);
      if (
        edited.snapshot.shareId !== input.shareId ||
        edited.snapshot.contentRevision <= input.baseRevision ||
        !sameOrderedIds(
          edited.snapshot.attachments.map(({ id }) => id),
          input.attachmentIds,
        )
      ) {
        return { status: "error" };
      }
      return { status: "ready", ...edited };
    } catch {
      return { status: "error" };
    }
  });

function setPrivateShareResponseHeaders() {
  setResponseHeader("Cache-Control", "private, no-store");
  setResponseHeader("Referrer-Policy", "no-referrer");
  setResponseHeader("X-Robots-Tag", "noindex, nofollow, noarchive, nosnippet");
}

async function loadMySharedNoteAccessRequest(
  shareId: string,
): Promise<SharedNoteAccessRequestResult> {
  const supabase = getSupabaseServerClient();
  const { data, error } = await supabase.rpc("get_my_session_access_request", {
    p_share_id: shareId,
  });
  if (error || !Array.isArray(data) || data.length > 1) {
    return { status: "error" };
  }
  if (data.length === 0) {
    return { status: "ready", request: null };
  }

  try {
    return {
      status: "ready",
      request: parseSessionAccessRequestState(data[0]),
    };
  } catch {
    return { status: "error" };
  }
}

function unavailableOrError(
  code: string | undefined,
): { status: "unavailable" } | { status: "error" } {
  return code === "22023" || code === "42501"
    ? { status: "unavailable" }
    : { status: "error" };
}

function apiBaseUrl() {
  return env.VITE_API_URL.endsWith("/")
    ? env.VITE_API_URL
    : `${env.VITE_API_URL}/`;
}

async function readBoundedJson(response: Response, maxBytes: number) {
  const contentLength = response.headers.get("content-length");
  if (
    contentLength &&
    (!/^\d+$/.test(contentLength) || Number(contentLength) > maxBytes)
  ) {
    return null;
  }

  const text = await response.text();
  if (new TextEncoder().encode(text).byteLength > maxBytes) return null;
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return null;
  }
}

function sameOrderedIds(left: string[], right: string[]) {
  return (
    left.length === right.length &&
    left.every((value, index) => value === right[index])
  );
}
