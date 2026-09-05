import type { SharedNoteDocument, SharedNoteNode } from "./shared-notes";

const UNSUPPORTED_INTERACTIVE_NODES = new Set(["clip"]);

export function hasUnsupportedSharedNoteInteractiveNode(
  document: SharedNoteDocument,
) {
  const stack: SharedNoteNode[] = [document];
  while (stack.length > 0) {
    const node = stack.pop();
    if (!node) continue;
    if (UNSUPPORTED_INTERACTIVE_NODES.has(node.type)) return true;
    if (node.content) stack.push(...node.content);
  }
  return false;
}
