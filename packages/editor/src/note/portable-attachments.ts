export type AttachmentContent = {
  type?: string;
  attrs?: Record<string, unknown>;
  content?: AttachmentContent[];
  marks?: { type: string; attrs?: Record<string, unknown> }[];
  text?: string;
};

export function normalizePortableAttachmentUrls<T extends AttachmentContent>(
  document: T,
): T {
  return normalizeNode(document) as T;
}

function normalizeNode(node: AttachmentContent): AttachmentContent {
  let contentChanged = false;
  const content = node.content?.map((child) => {
    const normalized = normalizeNode(child);
    contentChanged ||= normalized !== child;
    return normalized;
  });
  const attachmentId = node.attrs?.attachmentId;
  const isAttachment =
    (node.type === "image" || node.type === "fileAttachment") &&
    typeof attachmentId === "string" &&
    attachmentId.length > 0;
  const shouldNormalize =
    isAttachment &&
    (isLocalFileUrl(node.attrs?.src) ||
      (node.attrs !== undefined &&
        Object.prototype.hasOwnProperty.call(node.attrs, "path")));

  if (!shouldNormalize) {
    return contentChanged ? { ...node, content } : node;
  }

  const attrs = { ...node.attrs };
  if (isLocalFileUrl(attrs.src)) {
    delete attrs.src;
  }
  delete attrs.path;
  return {
    ...node,
    attrs,
    ...(contentChanged ? { content } : {}),
  };
}

function isLocalFileUrl(value: unknown): boolean {
  return (
    typeof value === "string" &&
    (value.startsWith("asset:") ||
      value.startsWith("file:") ||
      value.startsWith("http://asset.localhost/") ||
      value.startsWith("https://asset.localhost/"))
  );
}
