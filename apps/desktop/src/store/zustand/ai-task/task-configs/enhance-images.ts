import {
  type AttachmentInfo,
  commands as fsSyncCommands,
} from "@hypr/plugin-fs-sync";

export type EnhanceImageContext = {
  base64: string;
  mimeType: string;
  filename?: string;
};

type ImageReference = {
  attachmentId?: string;
  filename?: string;
  dataUrl?: { base64: string; mimeType: string };
};

const MAX_IMAGE_COUNT = 10;
const MAX_IMAGE_BYTES = 8 * 1024 * 1024;

const EXTENSION_TO_MIME: Record<string, string> = {
  gif: "image/gif",
  jpeg: "image/jpeg",
  jpg: "image/jpeg",
  png: "image/png",
  webp: "image/webp",
};

const MARKDOWN_IMAGE_RE = /!\[[^\]]*]\((<[^>]+>|[^)\s]+)(?:\s+"[^"]*")?\)/g;

export async function collectEnhanceImageContext(
  sessionId: string,
  rawContent: string | string[],
): Promise<EnhanceImageContext[]> {
  const references = Array.isArray(rawContent)
    ? rawContent.flatMap(collectImageReferences)
    : collectImageReferences(rawContent);
  const images: EnhanceImageContext[] = [];

  for (const ref of references) {
    if (!ref.dataUrl) {
      continue;
    }

    images.push(ref.dataUrl);
    if (images.length >= MAX_IMAGE_COUNT) {
      return images;
    }
  }

  const attachmentRefs = references.filter(
    (ref) => !ref.dataUrl && (ref.attachmentId || ref.filename),
  );
  if (attachmentRefs.length === 0) {
    return images;
  }

  const listResult = await fsSyncCommands.attachmentList(sessionId);
  if (listResult.status === "error") {
    console.warn(
      "[enhance] failed to list image attachments",
      listResult.error,
    );
    return images;
  }

  const attachmentsById = new Map(
    listResult.data.map((attachment) => [attachment.attachmentId, attachment]),
  );
  const attachmentsByFilename = new Map(
    listResult.data.map((attachment) => [
      getPathFilename(attachment.path) || attachment.attachmentId,
      attachment,
    ]),
  );
  const seen = new Set<string>();

  for (const ref of attachmentRefs) {
    const attachment =
      (ref.attachmentId ? attachmentsById.get(ref.attachmentId) : undefined) ??
      (ref.filename ? attachmentsById.get(ref.filename) : undefined) ??
      (ref.filename ? attachmentsByFilename.get(ref.filename) : undefined);

    if (!attachment || seen.has(attachment.attachmentId)) {
      continue;
    }

    seen.add(attachment.attachmentId);
    const image = await readImageAttachment(sessionId, attachment);
    if (!image) {
      continue;
    }

    images.push(image);
    if (images.length >= MAX_IMAGE_COUNT) {
      return images;
    }
  }

  return images;
}

export function collectImageReferences(rawContent: string): ImageReference[] {
  const trimmed = rawContent.trim();
  if (!trimmed) {
    return [];
  }

  if (trimmed.startsWith("{")) {
    try {
      return collectJsonImageReferences(JSON.parse(trimmed));
    } catch {
      return [];
    }
  }

  return collectMarkdownImageReferences(trimmed);
}

async function readImageAttachment(
  sessionId: string,
  attachment: AttachmentInfo,
): Promise<EnhanceImageContext | null> {
  const mimeType = getImageMimeType(attachment.extension);
  if (!mimeType) {
    return null;
  }

  const readResult = await fsSyncCommands.attachmentRead(
    sessionId,
    attachment.attachmentId,
  );
  if (readResult.status === "error") {
    console.warn("[enhance] failed to read image attachment", readResult.error);
    return null;
  }

  if (readResult.data.length > MAX_IMAGE_BYTES) {
    return null;
  }

  return {
    base64: bytesToBase64(readResult.data),
    mimeType,
    filename: attachment.attachmentId,
  };
}

function collectJsonImageReferences(node: unknown): ImageReference[] {
  const references: ImageReference[] = [];

  function visit(value: unknown) {
    if (!value || typeof value !== "object") {
      return;
    }

    const node = value as {
      type?: unknown;
      attrs?: Record<string, unknown>;
      content?: unknown[];
    };
    if (node.type === "image" || node.type === "fileAttachment") {
      const mimeType =
        typeof node.attrs?.mimeType === "string" ? node.attrs.mimeType : "";
      const src = typeof node.attrs?.src === "string" ? node.attrs.src : "";
      const isImage =
        node.type === "image" ||
        mimeType.startsWith("image/") ||
        !!getImageMimeType(getPathExtension(src));

      if (isImage) {
        references.push(referenceFromAttrs(node.attrs));
      }
    }

    node.content?.forEach(visit);
  }

  visit(node);
  return references;
}

function collectMarkdownImageReferences(markdown: string): ImageReference[] {
  const references: ImageReference[] = [];
  for (const match of markdown.matchAll(MARKDOWN_IMAGE_RE)) {
    const src = unwrapMarkdownUrl(match[1]);
    references.push(referenceFromSrc(src));
  }
  return references;
}

function referenceFromAttrs(
  attrs: Record<string, unknown> | undefined,
): ImageReference {
  const attachmentId =
    typeof attrs?.attachmentId === "string" && attrs.attachmentId.length > 0
      ? attrs.attachmentId
      : undefined;
  const src = typeof attrs?.src === "string" ? attrs.src : "";
  const path = typeof attrs?.path === "string" ? attrs.path : "";
  const srcRef = referenceFromSrc(src);

  return {
    attachmentId,
    dataUrl: srcRef.dataUrl,
    filename: srcRef.filename ?? getAttachmentFilename(path),
  };
}

function referenceFromSrc(src: string): ImageReference {
  const dataUrl = parseImageDataUrl(src);
  if (dataUrl) {
    return { dataUrl };
  }

  return { filename: getAttachmentFilename(src) };
}

function parseImageDataUrl(src: string): EnhanceImageContext | null {
  const match = src.match(
    /^data:(image\/(?:gif|jpe?g|png|webp));base64,(.+)$/i,
  );
  if (!match) {
    return null;
  }

  const mimeType =
    match[1].toLowerCase() === "image/jpg" ? "image/jpeg" : match[1];
  const base64 = match[2];
  if (getBase64ByteLength(base64) > MAX_IMAGE_BYTES) {
    return null;
  }

  return { base64, mimeType };
}

function unwrapMarkdownUrl(src: string): string {
  const unwrapped =
    src.startsWith("<") && src.endsWith(">") ? src.slice(1, -1) : src;
  return unwrapped.replace(/\\([()])/g, "$1");
}

function getImageMimeType(extension: string | undefined): string | null {
  if (!extension) {
    return null;
  }

  return EXTENSION_TO_MIME[extension.toLowerCase()] ?? null;
}

function getPathExtension(path: string): string | undefined {
  const filename = getPathFilename(path);
  const dotIndex = filename?.lastIndexOf(".") ?? -1;
  return dotIndex >= 0 ? filename?.slice(dotIndex + 1) : undefined;
}

function getPathFilename(path: string): string | undefined {
  const normalized = normalizePathLike(path);
  const filename = normalized.split(/[\\/]/).filter(Boolean).pop();
  return filename ? decodePathPart(filename) : undefined;
}

function getAttachmentFilename(src: string): string | undefined {
  const trimmed = src.trim();
  if (!trimmed) {
    return undefined;
  }

  try {
    const url = new URL(trimmed);
    if (url.protocol === "asset:" || url.protocol === "file:") {
      return getPathFilename(trimmed);
    }

    return undefined;
  } catch {}

  const normalized = normalizePathLike(trimmed);
  if (
    !normalized.includes("/attachments/") &&
    !normalized.includes("\\attachments\\")
  ) {
    return undefined;
  }

  return getPathFilename(normalized);
}

function normalizePathLike(path: string): string {
  const trimmed = path.trim();
  if (!trimmed) {
    return "";
  }

  try {
    const url = new URL(trimmed);
    if (url.protocol === "asset:" || url.protocol === "file:") {
      return decodePathPart(url.pathname);
    }
  } catch {}

  return decodePathPart(trimmed);
}

function decodePathPart(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

export function getBase64ByteLength(base64: string): number {
  const normalized = base64.replace(/\s/g, "");
  const padding = normalized.endsWith("==")
    ? 2
    : normalized.endsWith("=")
      ? 1
      : 0;
  return Math.floor((normalized.length * 3) / 4) - padding;
}

function bytesToBase64(bytes: number[]): string {
  let binary = "";
  const chunkSize = 0x8000;

  for (let i = 0; i < bytes.length; i += chunkSize) {
    binary += String.fromCharCode(...bytes.slice(i, i + chunkSize));
  }

  return btoa(binary);
}
