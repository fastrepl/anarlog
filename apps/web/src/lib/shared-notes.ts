import { z } from "zod";

const MAX_BODY_BYTES = 2 * 1024 * 1024;
const MAX_BODY_DEPTH = 64;
const MAX_BODY_NODES = 50_000;
const MAX_TITLE_BYTES = 4096;

export const shareIdSchema = z.string().uuid();
export const invitationIdSchema = z.string().uuid();
export const publicShareSlugSchema = z.string().regex(/^s_[0-9a-f]{32}$/);
export const handoffRequestIdSchema = z
  .string()
  .regex(
    /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
  );
export const sharedNoteDesktopSchemeSchema = z
  .enum(["hyprnote", "hyprnote-staging"])
  .catch("hyprnote")
  .default("hyprnote");
export type SharedNoteDesktopScheme = z.infer<
  typeof sharedNoteDesktopSchemeSchema
>;

export function buildSharedNoteWebPath(
  pathname: string,
  scheme: SharedNoteDesktopScheme = "hyprnote",
) {
  const parsedScheme = sharedNoteDesktopSchemeSchema.parse(scheme);
  return parsedScheme === "hyprnote-staging"
    ? `${pathname}?scheme=hyprnote-staging`
    : pathname;
}

export function buildAccountShareDeepLink(
  shareId: string,
  scheme: SharedNoteDesktopScheme = "hyprnote",
) {
  const parsedShareId = shareIdSchema.parse(shareId);
  const parsedScheme = sharedNoteDesktopSchemeSchema.parse(scheme);
  return `${parsedScheme}://share/open?mode=account&share_id=${parsedShareId}`;
}

export function buildShareHandoffDeepLink(
  requestId: string,
  scheme: SharedNoteDesktopScheme = "hyprnote",
) {
  const parsedRequestId = handoffRequestIdSchema.parse(requestId);
  const parsedScheme = sharedNoteDesktopSchemeSchema.parse(scheme);
  return `${parsedScheme}://share/open?mode=handoff&request_id=${parsedRequestId}`;
}

export type SharedNoteCapability = "viewer" | "commenter" | "editor";

export type SharedNoteMark = {
  type: string;
  attrs?: Record<string, string | number | boolean | null>;
};

export type SharedNoteNode = {
  type: string;
  attrs?: Record<string, string | number | boolean | null>;
  content?: SharedNoteNode[];
  marks?: SharedNoteMark[];
  text?: string;
};

export type SharedNoteDocument = SharedNoteNode & { type: "doc" };

export type SharedNoteSnapshot = {
  shareId: string;
  schemaVersion: 1;
  contentRevision: number;
  title: string;
  body: SharedNoteDocument;
  attachments: SharedNoteAttachment[];
  publishedAt: string;
};

export type SharedNoteWebEditSnapshot = {
  snapshot: SharedNoteSnapshot;
  accessVersion: number;
  webEditable: boolean;
};

export type SharedNoteAttachment = {
  id: string;
  filename: string;
  contentType: string;
  sizeBytes: number;
  sha256: string;
};

export type SharedNoteAttachmentDownload = SharedNoteAttachment & {
  signedUrl: string;
  expiresAt: string;
};

export type AuthenticatedSharedNote = {
  snapshot: SharedNoteSnapshot;
  capability: SharedNoteCapability;
  manageAccess: boolean;
  accessVersion: number;
  webEditable: boolean;
};

export type SharedNoteCommentAnchor = {
  quoteExact: string;
  quotePrefix: string;
  quoteSuffix: string;
  fromHint: number | null;
  toHint: number | null;
};

export type SharedNoteComment = {
  commentId: string;
  isAuthor: boolean;
  body: string;
  snapshotRevision: number;
  anchor: SharedNoteCommentAnchor | null;
  createdAt: string;
};

export type SharedNoteCommentCursor = {
  beforeCreatedAt: string;
  beforeCommentId: string;
};

export type SharedNoteCommentPage = {
  comments: SharedNoteComment[];
  nextCursor: SharedNoteCommentCursor | null;
};

export type SessionAccessRequestState = {
  requestId: string;
  requestedCapability: SharedNoteCapability;
  status: "pending" | "approved" | "denied" | "cancelled";
  createdAt: string;
  reviewedAt: string | null;
};

export type SessionInvitationState = {
  status: "pending" | "accepted" | "revoked" | "expired";
  capability: SharedNoteCapability;
  shareId: string | null;
};

export type SessionShareAccessEntry =
  | {
      entryType: "grant";
      entryId: string;
      userId: string;
      userEmail: string | null;
      capability: SharedNoteCapability;
      status: "active";
      createdAt: string;
      expiresAt: null;
    }
  | {
      entryType: "invitation";
      entryId: string;
      userId: string | null;
      userEmail: string;
      capability: SharedNoteCapability;
      status: "pending";
      createdAt: string;
      expiresAt: string;
    }
  | {
      entryType: "request";
      entryId: string;
      userId: string;
      userEmail: string;
      capability: SharedNoteCapability;
      status: "pending";
      createdAt: string;
      expiresAt: null;
    };

export type SessionShareAccessCursor = {
  beforeCreatedAt: string;
  beforeEntryId: string;
};

export type SessionShareAccessPage = {
  entries: SessionShareAccessEntry[];
  nextCursor: SessionShareAccessCursor | null;
};

export type ShareHandoff = {
  requestId: string;
  expiresAt: string;
};

const scalarAttributeSchema = z.union([
  z.string(),
  z.number(),
  z.boolean(),
  z.null(),
]);

const markSchema: z.ZodType<SharedNoteMark> = z
  .object({
    type: z.string().min(1).max(64),
    attrs: z.record(z.string(), scalarAttributeSchema).optional(),
  })
  .strip();

const nodeSchema: z.ZodType<SharedNoteNode> = z.lazy(() =>
  z
    .object({
      type: z.string().min(1).max(64),
      attrs: z.record(z.string(), scalarAttributeSchema).optional(),
      content: z.array(nodeSchema).optional(),
      marks: z.array(markSchema).max(16).optional(),
      text: z.string().optional(),
    })
    .strip(),
);

const gatewaySnapshotSchema = z
  .object({
    shareId: shareIdSchema,
    schemaVersion: z.literal(1),
    contentRevision: z.number().int().positive().safe(),
    title: z.string(),
    body: z.unknown(),
    attachments: z.array(z.unknown()).max(64),
    publishedAt: z.string(),
  })
  .strict();

const webEditSnapshotSchema = gatewaySnapshotSchema
  .extend({
    accessVersion: z.number().int().positive().safe(),
    webEditable: z.boolean(),
  })
  .strict();

const webEditConflictResponseSchema = z
  .object({
    code: z.literal("snapshot_conflict"),
    snapshot: z.unknown(),
  })
  .strict();

const authenticatedSnapshotRowSchema = z
  .object({
    share_id: shareIdSchema,
    schema_version: z.literal(1),
    content_revision: z.number().int().positive().safe(),
    title: z.string(),
    body_json: z.unknown(),
    attachments_json: z.array(z.unknown()).max(64),
    capability: z.enum(["viewer", "commenter", "editor"]),
    manage_access: z.boolean(),
    access_version: z.number().int().positive().safe(),
    web_editable: z.boolean(),
    published_at: z.string(),
  })
  .passthrough();

const sharedNoteCapabilitySchema = z.enum(["viewer", "commenter", "editor"]);
const rpcTimestampSchema = z.iso.datetime({ offset: true }).max(64);
const normalizedEmailSchema = z
  .string()
  .regex(/^[^\s@]+@[^\s@]+$/)
  .max(320)
  .refine((value) => value === value.trim().toLowerCase());

const sharedNoteCommentRowSchema = z
  .object({
    comment_id: shareIdSchema,
    is_author: z.boolean(),
    body: z.string().min(1).max(16384),
    snapshot_content_revision: z.number().int().positive().safe(),
    anchor_quote_exact: z.string().min(1).max(4096).nullish(),
    anchor_quote_prefix: z.string().max(256).nullish(),
    anchor_quote_suffix: z.string().max(256).nullish(),
    anchor_from_hint: z.number().int().positive().safe().nullish(),
    anchor_to_hint: z.number().int().positive().safe().nullish(),
    created_at: rpcTimestampSchema,
  })
  .strict()
  .refine(
    ({ anchor_quote_exact, anchor_quote_prefix, anchor_quote_suffix }) =>
      (anchor_quote_exact == null) === (anchor_quote_prefix == null) &&
      (anchor_quote_exact == null) === (anchor_quote_suffix == null),
  )
  .refine(
    ({ anchor_quote_exact, anchor_from_hint, anchor_to_hint }) =>
      (anchor_from_hint == null) === (anchor_to_hint == null) &&
      (anchor_from_hint == null ||
        (anchor_quote_exact != null && anchor_to_hint! > anchor_from_hint)),
  );

const sessionAccessRequestStateRowSchema = z
  .object({
    request_id: shareIdSchema,
    requested_capability: sharedNoteCapabilitySchema,
    status: z.enum(["pending", "approved", "denied", "cancelled"]),
    created_at: rpcTimestampSchema,
    reviewed_at: rpcTimestampSchema.nullable(),
  })
  .strict()
  .refine(({ reviewed_at, status }) =>
    status === "approved" || status === "denied"
      ? reviewed_at !== null
      : reviewed_at === null,
  );

const sessionInvitationStateRowSchema = z
  .object({
    status: z.enum(["pending", "accepted", "revoked", "expired"]),
    capability: sharedNoteCapabilitySchema,
    share_id: shareIdSchema.nullable(),
  })
  .strict();

const sessionShareAccessEntryRowSchema = z.discriminatedUnion("entry_type", [
  z
    .object({
      entry_type: z.literal("grant"),
      entry_id: shareIdSchema,
      user_id: shareIdSchema,
      user_email: normalizedEmailSchema.nullable(),
      capability: sharedNoteCapabilitySchema,
      status: z.literal("active"),
      created_at: rpcTimestampSchema,
      expires_at: z.null(),
    })
    .strict(),
  z
    .object({
      entry_type: z.literal("invitation"),
      entry_id: shareIdSchema,
      user_id: shareIdSchema.nullable(),
      user_email: normalizedEmailSchema,
      capability: sharedNoteCapabilitySchema,
      status: z.literal("pending"),
      created_at: rpcTimestampSchema,
      expires_at: rpcTimestampSchema,
    })
    .strict(),
  z
    .object({
      entry_type: z.literal("request"),
      entry_id: shareIdSchema,
      user_id: shareIdSchema,
      user_email: normalizedEmailSchema,
      capability: sharedNoteCapabilitySchema,
      status: z.literal("pending"),
      created_at: rpcTimestampSchema,
      expires_at: z.null(),
    })
    .strict(),
]);

const handoffSchema = z
  .object({
    requestId: handoffRequestIdSchema,
    expiresAt: z.string(),
  })
  .strict();

const attachmentSchema = z
  .object({
    id: shareIdSchema,
    filename: z.string().min(1).max(1024),
    contentType: z.string().min(1).max(512),
    sizeBytes: z
      .number()
      .int()
      .nonnegative()
      .max(512 * 1024 * 1024),
    sha256: z.string().regex(/^[0-9a-f]{64}$/),
  })
  .strict();

const attachmentDownloadSchema = attachmentSchema
  .extend({
    signedUrl: z.string().url().max(16_384),
    expiresAt: z.string(),
  })
  .strict();

export function parseGatewaySharedNote(value: unknown): SharedNoteSnapshot {
  const parsed = gatewaySnapshotSchema.parse(value);
  return {
    ...parsed,
    title: parseTitle(parsed.title),
    body: parseSharedNoteDocument(parsed.body),
    attachments: parseSharedNoteAttachments(parsed.attachments),
    publishedAt: parseTimestamp(parsed.publishedAt),
  };
}

export function parseSharedNoteWebEditSnapshot(
  value: unknown,
): SharedNoteWebEditSnapshot {
  const parsed = webEditSnapshotSchema.parse(value);
  const { accessVersion, webEditable, ...snapshot } = parsed;
  return {
    accessVersion,
    webEditable,
    snapshot: parseGatewaySharedNote(snapshot),
  };
}

export function parseSharedNoteWebEditConflict(
  value: unknown,
): SharedNoteWebEditSnapshot {
  const parsed = webEditConflictResponseSchema.parse(value);
  return parseSharedNoteWebEditSnapshot(parsed.snapshot);
}

export function parseAuthenticatedSharedNote(
  value: unknown,
): AuthenticatedSharedNote {
  const parsed = authenticatedSnapshotRowSchema.parse(value);
  return {
    capability: parsed.capability,
    manageAccess: parsed.manage_access,
    accessVersion: parsed.access_version,
    webEditable: parsed.web_editable,
    snapshot: {
      shareId: parsed.share_id,
      schemaVersion: parsed.schema_version,
      contentRevision: parsed.content_revision,
      title: parseTitle(parsed.title),
      body: parseSharedNoteDocument(parsed.body_json),
      attachments: parseSharedNoteAttachments(parsed.attachments_json),
      publishedAt: parseTimestamp(parsed.published_at),
    },
  };
}

export function parseSharedNoteComment(value: unknown): SharedNoteComment {
  const parsed = sharedNoteCommentRowSchema.parse(value);
  return {
    commentId: parsed.comment_id,
    isAuthor: parsed.is_author,
    body: parsed.body,
    snapshotRevision: parsed.snapshot_content_revision,
    anchor:
      parsed.anchor_quote_exact == null
        ? null
        : {
            quoteExact: parsed.anchor_quote_exact,
            quotePrefix: parsed.anchor_quote_prefix ?? "",
            quoteSuffix: parsed.anchor_quote_suffix ?? "",
            fromHint: parsed.anchor_from_hint ?? null,
            toHint: parsed.anchor_to_hint ?? null,
          },
    createdAt: parsed.created_at,
  };
}

export function parseSharedNoteCommentPage(
  value: unknown,
): SharedNoteCommentPage {
  if (!Array.isArray(value) || value.length > 101) {
    throw new Error("invalid shared-note comment page");
  }

  const parsed = value.map(parseSharedNoteComment);
  const descendingComments = parsed.slice(0, 100);
  const oldestComment = descendingComments.at(-1);
  return {
    comments: descendingComments.reverse(),
    nextCursor:
      parsed.length > descendingComments.length && oldestComment
        ? {
            beforeCreatedAt: oldestComment.createdAt,
            beforeCommentId: oldestComment.commentId,
          }
        : null,
  };
}

export function parseSessionAccessRequestState(
  value: unknown,
): SessionAccessRequestState {
  const parsed = sessionAccessRequestStateRowSchema.parse(value);
  return {
    requestId: parsed.request_id,
    requestedCapability: parsed.requested_capability,
    status: parsed.status,
    createdAt: parsed.created_at,
    reviewedAt: parsed.reviewed_at,
  };
}

export function parseSessionInvitationState(
  value: unknown,
): SessionInvitationState {
  const parsed = sessionInvitationStateRowSchema.parse(value);
  return {
    status: parsed.status,
    capability: parsed.capability,
    shareId: parsed.share_id,
  };
}

export function parseSessionShareAccessEntry(
  value: unknown,
): SessionShareAccessEntry {
  const parsed = sessionShareAccessEntryRowSchema.parse(value);
  if (parsed.entry_type === "grant") {
    return {
      entryType: parsed.entry_type,
      entryId: parsed.entry_id,
      userId: parsed.user_id,
      userEmail: parsed.user_email,
      capability: parsed.capability,
      status: parsed.status,
      createdAt: parsed.created_at,
      expiresAt: parsed.expires_at,
    };
  }
  if (parsed.entry_type === "invitation") {
    return {
      entryType: parsed.entry_type,
      entryId: parsed.entry_id,
      userId: parsed.user_id,
      userEmail: parsed.user_email,
      capability: parsed.capability,
      status: parsed.status,
      createdAt: parsed.created_at,
      expiresAt: parsed.expires_at,
    };
  }
  return {
    entryType: parsed.entry_type,
    entryId: parsed.entry_id,
    userId: parsed.user_id,
    userEmail: parsed.user_email,
    capability: parsed.capability,
    status: parsed.status,
    createdAt: parsed.created_at,
    expiresAt: parsed.expires_at,
  };
}

export function parseSessionShareAccessPage(
  value: unknown,
): SessionShareAccessPage {
  if (!Array.isArray(value) || value.length > 101) {
    throw new Error("invalid shared-note access page");
  }

  const parsed = value.map(parseSessionShareAccessEntry);
  const entries = parsed.slice(0, 100);
  const lastEntry = entries.at(-1);
  return {
    entries,
    nextCursor:
      parsed.length > entries.length && lastEntry
        ? {
            beforeCreatedAt: lastEntry.createdAt,
            beforeEntryId: lastEntry.entryId,
          }
        : null,
  };
}

export function parseSharedNoteAttachmentDownload(
  value: unknown,
): SharedNoteAttachmentDownload {
  const parsed = attachmentDownloadSchema.parse(value);
  const signedUrl = new URL(parsed.signedUrl);
  if (
    signedUrl.protocol !== "https:" ||
    signedUrl.username ||
    signedUrl.password ||
    signedUrl.hash
  ) {
    throw new Error("invalid shared-note attachment download");
  }
  return { ...parsed, expiresAt: parseTimestamp(parsed.expiresAt) };
}

function parseSharedNoteAttachments(value: unknown): SharedNoteAttachment[] {
  const parsed = z.array(attachmentSchema).max(64).parse(value);
  const ids = new Set<string>();
  for (const attachment of parsed) {
    if (ids.has(attachment.id)) {
      throw new Error("duplicate shared-note attachment");
    }
    ids.add(attachment.id);
  }
  return parsed;
}

export function parseShareHandoff(value: unknown): ShareHandoff {
  const parsed = handoffSchema.parse(value);
  return {
    requestId: parsed.requestId,
    expiresAt: parseTimestamp(parsed.expiresAt),
  };
}

export function parseSharedNoteDocument(value: unknown): SharedNoteDocument {
  validateDocumentBudget(value);
  const parsed = nodeSchema.parse(value);
  if (parsed.type !== "doc") {
    throw new Error("invalid shared note");
  }
  return parsed as SharedNoteDocument;
}

export function getSafeSharedNoteHref(value: unknown): string | null {
  if (typeof value !== "string") {
    return null;
  }

  try {
    const url = new URL(value.trim());
    if (
      (url.protocol === "http:" || url.protocol === "https:") &&
      url.hostname &&
      !url.username &&
      !url.password
    ) {
      return url.toString();
    }
    if (url.protocol === "mailto:" && url.pathname) {
      return url.toString();
    }
  } catch {
    return null;
  }

  return null;
}

export function getSharedNoteDescription(document: SharedNoteDocument) {
  const text = getSharedNotePlainText(document).replace(/\s+/g, " ").trim();
  return text.length <= 180 ? text : `${text.slice(0, 177).trimEnd()}…`;
}

export function getSharedNotePlainText(node: SharedNoteNode): string {
  const text = node.type === "text" ? (node.text ?? "") : "";
  const children = node.content?.map(getSharedNotePlainText).join(" ") ?? "";
  return `${text} ${children}`.trim();
}

export function withoutDuplicateLeadingTitle(
  document: SharedNoteDocument,
  title: string,
): SharedNoteDocument {
  const first = document.content?.[0];
  if (
    first?.type !== "heading" ||
    getSharedNotePlainText(first).trim() !== title.trim()
  ) {
    return document;
  }

  return {
    ...document,
    content: document.content?.slice(1),
  };
}

function parseTitle(value: string) {
  const title = value.trim();
  if (new TextEncoder().encode(title).byteLength > MAX_TITLE_BYTES) {
    throw new Error("invalid shared note");
  }
  return title;
}

function parseTimestamp(value: string) {
  if (!value || Number.isNaN(Date.parse(value))) {
    throw new Error("invalid shared note");
  }
  return value;
}

function validateDocumentBudget(value: unknown) {
  const encoded = JSON.stringify(value);
  if (new TextEncoder().encode(encoded).byteLength > MAX_BODY_BYTES) {
    throw new Error("invalid shared note");
  }

  const stack: Array<{ depth: number; value: unknown }> = [{ depth: 0, value }];
  let nodes = 0;

  while (stack.length > 0) {
    const current = stack.pop();
    if (
      !current ||
      current.depth > MAX_BODY_DEPTH ||
      !isRecord(current.value)
    ) {
      throw new Error("invalid shared note");
    }

    nodes += 1;
    if (nodes > MAX_BODY_NODES || typeof current.value.type !== "string") {
      throw new Error("invalid shared note");
    }

    if (current.value.content !== undefined) {
      if (!Array.isArray(current.value.content)) {
        throw new Error("invalid shared note");
      }
      for (const child of current.value.content) {
        stack.push({ depth: current.depth + 1, value: child });
      }
    }
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
