import type { Session, SupabaseClient } from "@supabase/supabase-js";

import type { JSONContent } from "@anlg/editor/note";

import type { SharedNoteAttachment } from "~/shared-notes/cache";

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const PUBLIC_SLUG_PATTERN = /^s_[0-9a-f]{32}$/;
const CAPABILITY_TOKEN_PATTERN = /^[A-Za-z0-9_-]{43}$/;
export const MAX_ACCESS_ROWS = 1_000;
export const MAX_RPC_DATA_BYTES = 1024 * 1024;
const MAX_COMMENT_BODY_BYTES = 16_384;
const MAX_COMMENT_ANCHOR_EXACT_BYTES = 4_096;
const MAX_COMMENT_ANCHOR_CONTEXT_BYTES = 256;
// callRpc rejects responses over MAX_RPC_DATA_BYTES; a comment row can
// approach 21 KiB (body + anchor quotes), so pages stay at 30 + 1 lookahead.
export const COMMENT_PAGE_SIZE = 30;
const MAX_SNAPSHOT_BODY_BYTES = 2 * 1024 * 1024;
export const MAX_SNAPSHOT_RESPONSE_BYTES = MAX_SNAPSHOT_BODY_BYTES + 256 * 1024;
const MAX_SNAPSHOT_TITLE_BYTES = 4_096;
const MAX_ACCESS_TOKEN_BYTES = 16 * 1024;
export const SNAPSHOT_TIMEOUT_MS = 10_000;

const scopes = ["restricted", "workspace", "link", "public"] as const;
export const settableScopes = ["restricted", "workspace", "public"] as const;
export const capabilities = ["viewer", "commenter", "editor"] as const;

export type SessionShareScope = (typeof scopes)[number];
export type SettableSessionShareScope = (typeof settableScopes)[number];
export type SessionAccessCapability = (typeof capabilities)[number];
export type ShareManagementContext = {
  supabase: SupabaseClient;
  session: Session;
  signal?: AbortSignal;
};

export type CreatedSessionShare = {
  shareId: string;
  generalScope: SessionShareScope;
  publicSlug: string;
  accessVersion: number;
  wasCreated: boolean;
};

export type SessionShareManagement = {
  shareId: string;
  workspaceId: string;
  sessionId: string;
  generalScope: SessionShareScope;
  generalWorkspaceId: string | null;
  publicSlug: string;
  hasActiveLink: boolean;
  accessVersion: number;
};

export type SessionShareDeletionResult =
  | {
      shareId: null;
      accessVersion: null;
      deletedAt: null;
      wasDeleted: false;
    }
  | {
      shareId: string;
      accessVersion: number;
      deletedAt: string;
      wasDeleted: boolean;
    };

export type SessionShareScopeResult = {
  shareId: string;
  generalScope: SettableSessionShareScope;
  generalWorkspaceId: string | null;
  publicSlug: string;
  accessVersion: number;
};

export type SessionShareLinkResult = {
  shareId: string;
  linkId: string;
  linkToken: string | null;
  accessVersion: number;
  wasCreated: boolean;
};

export type SessionAccessInvitationResult = {
  invitationId: string;
  inviteToken: string | null;
  invitationExpiresAt: string;
  wasCreated: boolean;
};

export type SendSessionAccessInvitationEmailInput = {
  apiBaseUrl: string;
  session: Session;
  shareId: string;
  invitationId: string;
  inviteToken: string;
  noteTitle: string;
  signal?: AbortSignal;
  fetcher?: typeof fetch;
};

export type SessionShareAccessEntry =
  | {
      entryType: "grant";
      entryId: string;
      userId: string;
      userEmail: string | null;
      capability: SessionAccessCapability;
      status: "active";
      createdAt: string;
      expiresAt: null;
    }
  | {
      entryType: "invitation";
      entryId: string;
      userId: string | null;
      userEmail: string;
      capability: SessionAccessCapability;
      status: "pending";
      createdAt: string;
      expiresAt: string;
    }
  | {
      entryType: "request";
      entryId: string;
      userId: string | null;
      userEmail: string | null;
      capability: SessionAccessCapability;
      status: "pending";
      createdAt: string;
      expiresAt: null;
    };

export type SessionShareCommentAnchor = {
  quoteExact: string;
  quotePrefix: string;
  quoteSuffix: string;
  fromHint: number | null;
  toHint: number | null;
};

export type SessionShareComment = {
  commentId: string;
  isAuthor: boolean;
  snapshotContentRevision: number;
  body: string;
  anchor: SessionShareCommentAnchor | null;
  createdAt: string;
};

export type SessionShareCommentPage = {
  comments: SessionShareComment[];
  nextCursor: { beforeCreatedAt: string; beforeCommentId: string } | null;
};

export type PublishedSessionShareSnapshot = {
  shareId: string;
  schemaVersion: 1;
  contentRevision: number;
  title: string;
  body: JSONContent;
  attachments: SharedNoteAttachment[];
  webEditable: boolean;
  accessVersion: number;
  publishedAt: string;
};

export type PublishSessionShareSnapshotInput = {
  apiBaseUrl: string;
  session: Session;
  shareId: string;
  baseRevision: number;
  mutationId: string;
  title: string;
  body: unknown;
  attachmentIds?: string[];
  signal?: AbortSignal;
  fetcher?: typeof fetch;
};

export class ShareManagementError extends Error {
  constructor() {
    super("Share management is unavailable");
    this.name = "ShareManagementError";
  }
}

export function parseSessionShareDocument(value: unknown): JSONContent {
  try {
    let parsed: unknown;
    if (typeof value === "string") {
      if (utf8Length(value) > MAX_SNAPSHOT_BODY_BYTES) {
        throw unavailable();
      }
      parsed = JSON.parse(value);
    } else {
      const encoded = JSON.stringify(value);
      if (utf8Length(encoded) > MAX_SNAPSHOT_BODY_BYTES) {
        throw unavailable();
      }
      parsed = JSON.parse(encoded);
    }
    if (
      !isRecord(parsed) ||
      parsed.type !== "doc" ||
      (parsed.content !== undefined && !Array.isArray(parsed.content))
    ) {
      throw unavailable();
    }
    return parsed as JSONContent;
  } catch (error) {
    if (error instanceof ShareManagementError) {
      throw error;
    }
    throw unavailable();
  }
}

export function parseCreatedSessionShare(value: unknown): CreatedSessionShare {
  const row = expectRecord(value, [
    "share_id",
    "general_scope",
    "public_slug",
    "access_version",
    "was_created",
  ]);
  return {
    shareId: expectUuid(row.share_id),
    generalScope: expectScope(row.general_scope),
    publicSlug: expectPublicSlug(row.public_slug),
    accessVersion: expectPositiveInteger(row.access_version),
    wasCreated: expectBoolean(row.was_created),
  };
}

export function parseSessionShareManagement(
  value: unknown,
): SessionShareManagement {
  const row = expectRecord(value, [
    "share_id",
    "workspace_id",
    "session_id",
    "general_scope",
    "general_workspace_id",
    "public_slug",
    "has_active_link",
    "access_version",
  ]);
  const generalScope = expectScope(row.general_scope);
  const generalWorkspaceId = expectNullableUuid(row.general_workspace_id);
  if ((generalScope === "workspace") !== (generalWorkspaceId !== null)) {
    throw unavailable();
  }
  return {
    shareId: expectUuid(row.share_id),
    workspaceId: expectUuid(row.workspace_id),
    sessionId: expectSessionId(row.session_id),
    generalScope,
    generalWorkspaceId,
    publicSlug: expectPublicSlug(row.public_slug),
    hasActiveLink: expectBoolean(row.has_active_link),
    accessVersion: expectPositiveInteger(row.access_version),
  };
}

export function parseSessionShareDeletionResult(
  value: unknown,
): SessionShareDeletionResult {
  const row = expectRecord(value, [
    "share_id",
    "access_version",
    "deleted_at",
    "was_deleted",
  ]);
  const wasDeleted = expectBoolean(row.was_deleted);
  if (row.share_id === null) {
    if (wasDeleted || row.access_version !== null || row.deleted_at !== null) {
      throw unavailable();
    }
    return {
      shareId: null,
      accessVersion: null,
      deletedAt: null,
      wasDeleted: false,
    };
  }
  return {
    shareId: expectUuid(row.share_id),
    accessVersion: expectPositiveInteger(row.access_version),
    deletedAt: expectTimestamp(row.deleted_at),
    wasDeleted,
  };
}

export function parseSessionShareScopeResult(
  value: unknown,
): SessionShareScopeResult {
  const row = expectRecord(value, [
    "share_id",
    "general_scope",
    "general_workspace_id",
    "public_slug",
    "access_version",
  ]);
  const generalScope = expectOneOf(row.general_scope, settableScopes);
  const generalWorkspaceId = expectNullableUuid(row.general_workspace_id);
  if ((generalScope === "workspace") !== (generalWorkspaceId !== null)) {
    throw unavailable();
  }
  return {
    shareId: expectUuid(row.share_id),
    generalScope,
    generalWorkspaceId,
    publicSlug: expectPublicSlug(row.public_slug),
    accessVersion: expectPositiveInteger(row.access_version),
  };
}

export function parseSessionShareLinkResult(
  value: unknown,
): SessionShareLinkResult {
  const row = expectRecord(value, [
    "share_id",
    "link_id",
    "link_token",
    "access_version",
    "was_created",
  ]);
  const wasCreated = expectBoolean(row.was_created);
  const linkToken =
    row.link_token === null ? null : expectCapabilityToken(row.link_token);
  if (wasCreated !== (linkToken !== null)) {
    throw unavailable();
  }
  return {
    shareId: expectUuid(row.share_id),
    linkId: expectUuid(row.link_id),
    linkToken,
    accessVersion: expectPositiveInteger(row.access_version),
    wasCreated,
  };
}

export function parseSessionAccessInvitationResult(
  value: unknown,
): SessionAccessInvitationResult {
  const row = expectRecord(value, [
    "invitation_id",
    "invite_token",
    "invitation_expires_at",
    "was_created",
  ]);
  const wasCreated = expectBoolean(row.was_created);
  const inviteToken =
    row.invite_token === null ? null : expectCapabilityToken(row.invite_token);
  if (wasCreated !== (inviteToken !== null)) {
    throw unavailable();
  }
  return {
    invitationId: expectUuid(row.invitation_id),
    inviteToken,
    invitationExpiresAt: expectTimestamp(row.invitation_expires_at),
    wasCreated,
  };
}

export function parseSessionShareAccessEntry(
  value: unknown,
): SessionShareAccessEntry {
  const row = expectRecord(value, [
    "entry_type",
    "entry_id",
    "user_id",
    "user_email",
    "capability",
    "status",
    "created_at",
    "expires_at",
  ]);
  const common = {
    entryId: expectUuid(row.entry_id),
    capability: expectCapability(row.capability),
    createdAt: expectTimestamp(row.created_at),
  };
  if (row.entry_type === "grant") {
    if (row.status !== "active" || row.expires_at !== null) {
      throw unavailable();
    }
    return {
      entryType: "grant",
      ...common,
      userId: expectUuid(row.user_id),
      userEmail: expectNullableEmail(row.user_email),
      status: "active",
      expiresAt: null,
    };
  }
  if (row.entry_type === "invitation") {
    if (row.status !== "pending") {
      throw unavailable();
    }
    return {
      entryType: "invitation",
      ...common,
      userId: expectNullableUuid(row.user_id),
      userEmail: expectEmail(row.user_email),
      status: "pending",
      expiresAt: expectTimestamp(row.expires_at),
    };
  }
  if (
    row.entry_type !== "request" ||
    row.status !== "pending" ||
    row.expires_at !== null
  ) {
    throw unavailable();
  }
  return {
    entryType: "request",
    ...common,
    userId: expectNullableUuid(row.user_id),
    userEmail: expectNullableEmail(row.user_email),
    status: "pending",
    expiresAt: null,
  };
}

export function parseSessionShareComment(value: unknown): SessionShareComment {
  const row = expectRecord(value, [
    "comment_id",
    "is_author",
    "snapshot_content_revision",
    "body",
    "anchor_quote_exact",
    "anchor_quote_prefix",
    "anchor_quote_suffix",
    "anchor_from_hint",
    "anchor_to_hint",
    "created_at",
  ]);
  return {
    commentId: expectUuid(row.comment_id),
    isAuthor: expectBoolean(row.is_author),
    snapshotContentRevision: expectPositiveInteger(
      row.snapshot_content_revision,
    ),
    body: expectCommentBody(row.body),
    anchor: parseCommentAnchorColumns(row),
    createdAt: expectTimestamp(row.created_at),
  };
}

function parseCommentAnchorColumns(
  row: Record<string, unknown>,
): SessionShareCommentAnchor | null {
  if (row.anchor_quote_exact === null) {
    if (
      row.anchor_quote_prefix !== null ||
      row.anchor_quote_suffix !== null ||
      row.anchor_from_hint !== null ||
      row.anchor_to_hint !== null
    ) {
      throw unavailable();
    }
    return null;
  }
  if (row.anchor_quote_prefix === null || row.anchor_quote_suffix === null) {
    throw unavailable();
  }
  return {
    quoteExact: expectAnchorQuote(row.anchor_quote_exact),
    quotePrefix: expectAnchorContext(row.anchor_quote_prefix),
    quoteSuffix: expectAnchorContext(row.anchor_quote_suffix),
    ...expectAnchorHints(row.anchor_from_hint, row.anchor_to_hint),
  };
}

export function assertCommentAnchor(anchor: SessionShareCommentAnchor) {
  expectAnchorQuote(anchor.quoteExact);
  expectAnchorContext(anchor.quotePrefix);
  expectAnchorContext(anchor.quoteSuffix);
  expectAnchorHints(anchor.fromHint, anchor.toHint);
}

function expectAnchorQuote(value: unknown) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    utf8Length(value) > MAX_COMMENT_ANCHOR_EXACT_BYTES
  ) {
    throw unavailable();
  }
  return value;
}

function expectAnchorContext(value: unknown) {
  if (
    typeof value !== "string" ||
    utf8Length(value) > MAX_COMMENT_ANCHOR_CONTEXT_BYTES
  ) {
    throw unavailable();
  }
  return value;
}

function expectAnchorHints(
  fromHint: unknown,
  toHint: unknown,
): { fromHint: number | null; toHint: number | null } {
  if (fromHint === null && toHint === null) {
    return { fromHint: null, toHint: null };
  }
  if (
    !Number.isSafeInteger(fromHint) ||
    !Number.isSafeInteger(toHint) ||
    (fromHint as number) < 1 ||
    (toHint as number) <= (fromHint as number)
  ) {
    throw unavailable();
  }
  return { fromHint: fromHint as number, toHint: toHint as number };
}

export function normalizeCommentBody(value: string) {
  if (typeof value !== "string") {
    throw unavailable();
  }
  return expectCommentBody(value.trim());
}

function expectCommentBody(value: unknown) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    utf8Length(value) > MAX_COMMENT_BODY_BYTES
  ) {
    throw unavailable();
  }
  return value;
}

export function parsePublishedSessionShareSnapshot(
  value: unknown,
): PublishedSessionShareSnapshot {
  const row = expectRecord(value, [
    "shareId",
    "schemaVersion",
    "contentRevision",
    "title",
    "body",
    "attachments",
    "webEditable",
    "accessVersion",
    "publishedAt",
  ]);
  if (row.schemaVersion !== 1) {
    throw unavailable();
  }
  return {
    shareId: expectUuid(row.shareId),
    schemaVersion: 1,
    contentRevision: expectPositiveInteger(row.contentRevision),
    title: expectTitle(row.title),
    body: parseSessionShareDocument(row.body),
    attachments: parseSharedNoteAttachments(row.attachments),
    webEditable: expectBoolean(row.webEditable),
    accessVersion: expectPositiveInteger(row.accessVersion),
    publishedAt: expectTimestamp(row.publishedAt),
  };
}

export function normalizeAttachmentIds(value: string[]) {
  if (!Array.isArray(value) || value.length > 64) throw unavailable();
  const ids = value.map(expectUuid);
  if (new Set(ids).size !== ids.length) throw unavailable();
  return ids;
}

function parseSharedNoteAttachments(value: unknown): SharedNoteAttachment[] {
  if (!Array.isArray(value) || value.length > 64) throw unavailable();
  const ids = new Set<string>();
  return value.map((candidate) => {
    const row = expectRecord(candidate, [
      "id",
      "filename",
      "contentType",
      "sizeBytes",
      "sha256",
    ]);
    const id = expectUuid(row.id);
    if (ids.has(id)) throw unavailable();
    ids.add(id);
    if (
      typeof row.filename !== "string" ||
      row.filename.length === 0 ||
      row.filename.trim() !== row.filename ||
      utf8Length(row.filename) > 1024 ||
      /[\\/\u0000-\u001f\u007f]/.test(row.filename) ||
      typeof row.contentType !== "string" ||
      row.contentType.length === 0 ||
      row.contentType.length > 255 ||
      !Number.isSafeInteger(row.sizeBytes) ||
      (row.sizeBytes as number) < 1 ||
      (row.sizeBytes as number) > 512 * 1024 * 1024 ||
      typeof row.sha256 !== "string" ||
      !/^[0-9a-f]{64}$/.test(row.sha256)
    ) {
      throw unavailable();
    }
    return {
      id,
      filename: row.filename,
      contentType: row.contentType,
      sizeBytes: row.sizeBytes as number,
      sha256: row.sha256,
    };
  });
}

export function singleRow(value: unknown) {
  if (!Array.isArray(value) || value.length !== 1) {
    throw unavailable();
  }
  return value[0];
}

export function expectRecord(value: unknown, expectedKeys: readonly string[]) {
  if (!isRecord(value)) {
    throw unavailable();
  }
  const keys = Object.keys(value);
  if (
    keys.length !== expectedKeys.length ||
    expectedKeys.some(
      (key) => !Object.prototype.hasOwnProperty.call(value, key),
    )
  ) {
    throw unavailable();
  }
  return value;
}

export function assertAuthenticatedSession(session: Session) {
  if (
    session.user.is_anonymous === true ||
    !UUID_PATTERN.test(session.user.id) ||
    typeof session.access_token !== "string" ||
    session.access_token === "" ||
    /[\u0000-\u001f\u007f]/.test(session.access_token) ||
    utf8Length(session.access_token) > MAX_ACCESS_TOKEN_BYTES
  ) {
    throw unavailable();
  }
}

export function assertSessionId(value: unknown): asserts value is string {
  expectSessionId(value);
}

function expectSessionId(value: unknown) {
  if (
    typeof value !== "string" ||
    value === "" ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/.test(value) ||
    utf8Length(value) > 128
  ) {
    throw unavailable();
  }
  return value;
}

export function assertUuid(value: unknown): asserts value is string {
  expectUuid(value);
}

export function expectUuid(value: unknown) {
  if (typeof value !== "string" || !UUID_PATTERN.test(value)) {
    throw unavailable();
  }
  return value;
}

function expectNullableUuid(value: unknown) {
  return value === null ? null : expectUuid(value);
}

function expectPublicSlug(value: unknown) {
  if (typeof value !== "string" || !PUBLIC_SLUG_PATTERN.test(value)) {
    throw unavailable();
  }
  return value;
}

function expectScope(value: unknown) {
  return expectOneOf(value, scopes);
}

export function expectCapability(value: unknown) {
  return expectOneOf(value, capabilities);
}

export function assertOneOf<T extends string>(
  value: unknown,
  values: readonly T[],
): asserts value is T {
  expectOneOf(value, values);
}

function expectOneOf<T extends string>(value: unknown, values: readonly T[]) {
  if (typeof value !== "string" || !values.includes(value as T)) {
    throw unavailable();
  }
  return value as T;
}

export function expectPositiveInteger(value: unknown) {
  if (!Number.isSafeInteger(value) || (value as number) < 1) {
    throw unavailable();
  }
  return value as number;
}

function expectBoolean(value: unknown) {
  if (typeof value !== "boolean") {
    throw unavailable();
  }
  return value;
}

export function expectCapabilityToken(value: unknown) {
  if (typeof value !== "string" || !CAPABILITY_TOKEN_PATTERN.test(value)) {
    throw unavailable();
  }
  return value;
}

export function expectTimestamp(value: unknown) {
  if (typeof value !== "string" || !Number.isFinite(Date.parse(value))) {
    throw unavailable();
  }
  return value;
}

export function normalizeEmail(value: string) {
  if (typeof value !== "string") {
    throw unavailable();
  }
  return expectEmail(value.trim().toLowerCase());
}

function expectEmail(value: unknown) {
  if (
    typeof value !== "string" ||
    value.trim() !== value ||
    value.toLowerCase() !== value ||
    !/^[^\s@]+@[^\s@]+$/.test(value) ||
    /[\u0000-\u001f\u007f]/.test(value) ||
    utf8Length(value) > 320
  ) {
    throw unavailable();
  }
  return value;
}

function expectNullableEmail(value: unknown) {
  return value === null ? null : expectEmail(value);
}

export function normalizeTitle(value: string) {
  if (typeof value !== "string") {
    throw unavailable();
  }
  return expectTitle(value.trim());
}

function expectTitle(value: unknown) {
  if (
    typeof value !== "string" ||
    value.trim() !== value ||
    utf8Length(value) > MAX_SNAPSHOT_TITLE_BYTES
  ) {
    throw unavailable();
  }
  return value;
}

export function assertJsonSize(value: unknown, limit: number) {
  try {
    if (utf8Length(JSON.stringify(value)) > limit) {
      throw unavailable();
    }
  } catch (error) {
    if (error instanceof ShareManagementError) {
      throw error;
    }
    throw unavailable();
  }
}

export function utf8Length(value: string) {
  return new TextEncoder().encode(value).byteLength;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function unavailable(): ShareManagementError {
  return new ShareManagementError();
}
