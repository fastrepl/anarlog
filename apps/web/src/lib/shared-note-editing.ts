import type {
  AuthenticatedSharedNote,
  SharedNoteDocument,
  SharedNoteNode,
  SharedNoteSnapshot,
} from "./shared-notes";

const UNSUPPORTED_WEB_EDITOR_NODES = new Set([
  "clip",
  "fileAttachment",
  "image",
]);

export type SharedNoteWebEditInput = {
  shareId: string;
  baseRevision: number;
  mutationId: string;
  title: string;
  body: SharedNoteDocument;
  attachmentIds: string[];
};

export function canEditSharedNoteOnWeb(
  note: Pick<AuthenticatedSharedNote, "capability" | "webEditable"> | null,
) {
  return note?.capability === "editor" && note.webEditable;
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
