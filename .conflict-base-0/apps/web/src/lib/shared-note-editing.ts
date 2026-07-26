import type {
  AuthenticatedSharedNote,
  SharedNoteDocument,
  SharedNoteNode,
  SharedNoteSnapshot,
} from "./shared-notes";

const UNSUPPORTED_WEB_EDITOR_NODES = new Set(["clip"]);
const SHARED_ATTACHMENT_NODES = new Set(["fileAttachment", "image"]);

export type SharedNoteWebEditInput = {
  shareId: string;
  baseRevision: number;
  mutationId: string;
  title: string;
  body: SharedNoteDocument;
  attachmentIds: string[];
};

export type SharedNoteViewerAuthorization = {
  note: AuthenticatedSharedNote | null;
  state: "ready" | "access_changed" | "sign_in_required";
};

export function canEditSharedNoteOnWeb(
  note: Pick<AuthenticatedSharedNote, "capability" | "webEditable"> | null,
) {
  return note?.capability === "editor" && note.webEditable;
}

export function syncSharedNoteViewerAuthorization(
  current: SharedNoteViewerAuthorization,
  note: AuthenticatedSharedNote | null,
): SharedNoteViewerAuthorization {
  return current.state === "sign_in_required" ? current : { ...current, note };
}

export function resolveSharedNoteViewerAuthorization(
  note: AuthenticatedSharedNote | null,
): SharedNoteViewerAuthorization {
  return {
    note,
    state: note?.capability === "editor" ? "ready" : "access_changed",
  };
}

export function getSharedNoteWebEditPreparationMessage(
  note: Pick<AuthenticatedSharedNote, "capability" | "webEditable"> | null,
  hasUnsupportedContent: boolean,
) {
  if (
    note?.capability !== "editor" ||
    (note.webEditable && !hasUnsupportedContent)
  ) {
    return null;
  }
  return "This note needs to be prepared before it can be edited on the web. You can still edit it in the Anarlog app.";
}

export function shouldRenderSharedNoteUnavailable({
  accessRevoked,
  hasFallbackSnapshot,
  revokedBehavior,
}: {
  accessRevoked: boolean;
  hasFallbackSnapshot: boolean;
  revokedBehavior: "read-only" | "unavailable";
}) {
  return (
    accessRevoked && (revokedBehavior === "unavailable" || !hasFallbackSnapshot)
  );
}

export function getSharedNoteReadOnlySnapshot(
  current: SharedNoteSnapshot,
  fallback: SharedNoteSnapshot | null | undefined,
): SharedNoteSnapshot | null {
  if (!fallback || fallback.shareId !== current.shareId) {
    return null;
  }
  return fallback.contentRevision > current.contentRevision
    ? fallback
    : current;
}

export function hasUnsupportedSharedNoteEditorNode(
  document: SharedNoteDocument,
) {
  const stack: SharedNoteNode[] = [document];
  while (stack.length > 0) {
    const node = stack.pop();
    if (!node) continue;
    if (UNSUPPORTED_WEB_EDITOR_NODES.has(node.type)) return true;
    if (node.content) stack.push(...node.content);
  }
  return false;
}

export function canonicalizeSharedNoteWebDraft(
  document: SharedNoteDocument,
  attachmentIds: readonly string[],
): SharedNoteDocument | null {
  const canonical = canonicalizeSharedNoteWebDraftNode(
    document,
    new Set(attachmentIds),
  );
  return canonical?.type === "doc" ? (canonical as SharedNoteDocument) : null;
}

export function ensureSharedNoteEditorTitle(
  document: SharedNoteDocument,
  title: string,
): SharedNoteDocument {
  if (isTitleHeading(document.content?.[0])) return document;

  const heading: SharedNoteNode = {
    type: "heading",
    attrs: { level: 1 },
    ...(title ? { content: [{ type: "text", text: title }] } : {}),
  };
  return {
    ...document,
    content: [heading, ...(document.content ?? [])],
  };
}

export function deriveSharedNoteEditorTitle(document: SharedNoteDocument) {
  const first = document.content?.[0];
  return isTitleHeading(first) ? getInlineText(first).trim() : "";
}

export function buildSharedNoteWebEditInput({
  body,
  mutationId,
  snapshot,
}: {
  body: SharedNoteDocument;
  mutationId: string;
  snapshot: SharedNoteSnapshot;
}): SharedNoteWebEditInput {
  return {
    shareId: snapshot.shareId,
    baseRevision: snapshot.contentRevision,
    mutationId,
    title: deriveSharedNoteEditorTitle(body),
    body,
    attachmentIds: snapshot.attachments.map(({ id }) => id),
  };
}

export function reuseSharedNoteMutationIdForUnchangedDraft(
  input: SharedNoteWebEditInput,
  previousInput: SharedNoteWebEditInput | undefined,
) {
  if (!previousInput || !sameSharedNoteDraft(input, previousInput)) {
    return input;
  }
  return { ...input, mutationId: previousInput.mutationId };
}

function sameSharedNoteDraft(
  left: SharedNoteWebEditInput,
  right: SharedNoteWebEditInput,
) {
  return (
    left.shareId === right.shareId &&
    left.baseRevision === right.baseRevision &&
    left.title === right.title &&
    left.attachmentIds.length === right.attachmentIds.length &&
    left.attachmentIds.every(
      (id, index) => id === right.attachmentIds[index],
    ) &&
    JSON.stringify(left.body) === JSON.stringify(right.body)
  );
}

function isTitleHeading(
  node: SharedNoteNode | undefined,
): node is SharedNoteNode {
  return node?.type === "heading" && (node.attrs?.level ?? 1) === 1;
}

function getInlineText(node: SharedNoteNode): string {
  if (node.type === "text") return node.text ?? "";
  return node.content?.map(getInlineText).join("") ?? "";
}

function canonicalizeSharedNoteWebDraftNode(
  node: SharedNoteNode,
  attachmentIds: ReadonlySet<string>,
): SharedNoteNode | null {
  if (UNSUPPORTED_WEB_EDITOR_NODES.has(node.type)) return null;

  if (SHARED_ATTACHMENT_NODES.has(node.type)) {
    const sharedAttachmentId = node.attrs?.sharedAttachmentId;
    if (
      typeof sharedAttachmentId !== "string" ||
      !attachmentIds.has(sharedAttachmentId)
    ) {
      return null;
    }
    return {
      type: node.type,
      attrs: { sharedAttachmentId },
    };
  }

  if (!node.content) return node;

  const content: SharedNoteNode[] = [];
  for (const child of node.content) {
    const canonical = canonicalizeSharedNoteWebDraftNode(child, attachmentIds);
    if (!canonical) return null;
    content.push(canonical);
  }
  return content.every((child, index) => child === node.content?.[index])
    ? node
    : { ...node, content };
}
