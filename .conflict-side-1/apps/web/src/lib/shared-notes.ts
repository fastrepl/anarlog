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
  publishedAt: string;
};

export type AuthenticatedSharedNote = {
  snapshot: SharedNoteSnapshot;
  capability: SharedNoteCapability;
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
    contentRevision: z.number().int().positive(),
    title: z.string(),
    body: z.unknown(),
    publishedAt: z.string(),
  })
  .strict();

const authenticatedSnapshotRowSchema = z
  .object({
    share_id: shareIdSchema,
    schema_version: z.literal(1),
    content_revision: z.number().int().positive(),
    title: z.string(),
    body_json: z.unknown(),
    capability: z.enum(["viewer", "commenter", "editor"]),
    published_at: z.string(),
  })
  .passthrough();

const handoffSchema = z
  .object({
    requestId: handoffRequestIdSchema,
    expiresAt: z.string(),
  })
  .strict();

export function parseGatewaySharedNote(value: unknown): SharedNoteSnapshot {
  const parsed = gatewaySnapshotSchema.parse(value);
  return {
    ...parsed,
    title: parseTitle(parsed.title),
    body: parseSharedNoteDocument(parsed.body),
    publishedAt: parseTimestamp(parsed.publishedAt),
  };
}

export function parseAuthenticatedSharedNote(
  value: unknown,
): AuthenticatedSharedNote {
  const parsed = authenticatedSnapshotRowSchema.parse(value);
  return {
    capability: parsed.capability,
    snapshot: {
      shareId: parsed.share_id,
      schemaVersion: parsed.schema_version,
      contentRevision: parsed.content_revision,
      title: parseTitle(parsed.title),
      body: parseSharedNoteDocument(parsed.body_json),
      publishedAt: parseTimestamp(parsed.published_at),
    },
  };
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
