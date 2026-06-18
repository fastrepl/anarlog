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

type IndexedImageReference = ImageReference & {
  index: number;
};

const MAX_IMAGE_COUNT = 10;
const MAX_IMAGE_BYTES = 128 * 1024;
const MAX_TOTAL_IMAGE_BYTES = 768 * 1024;
const MAX_SOURCE_IMAGE_BYTES = 8 * 1024 * 1024;
const COMPRESSED_IMAGE_MIME_TYPE = "image/jpeg";
const MAX_COMPRESSED_IMAGE_EDGE = 1280;
const MIN_COMPRESSED_IMAGE_EDGE = 512;
const COMPRESSED_IMAGE_QUALITY_STEPS = [0.82, 0.72, 0.62, 0.52];

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
  const candidateRefs = getCandidateImageReferences(references);
  const images: Array<{ index: number; image: EnhanceImageContext }> = [];
  let totalImageBytes = 0;
  const seen = new Set<string>();
  let attachmentLookup:
    | {
        byId: Map<string, AttachmentInfo>;
        byFilename: Map<string, AttachmentInfo>;
      }
    | null
    | undefined;

  async function getAttachmentLookup() {
    if (attachmentLookup !== undefined) {
      return attachmentLookup;
    }

    const listResult = await fsSyncCommands.attachmentList(sessionId);
    if (listResult.status === "error") {
      console.warn(
        "[enhance] failed to list image attachments",
        listResult.error,
      );
      attachmentLookup = null;
      return attachmentLookup;
    }

    attachmentLookup = {
      byId: new Map(
        listResult.data.map((attachment) => [
          attachment.attachmentId,
          attachment,
        ]),
      ),
      byFilename: new Map(
        listResult.data.map((attachment) => [
          getPathFilename(attachment.path) || attachment.attachmentId,
          attachment,
        ]),
      ),
    };
    return attachmentLookup;
  }

  for (const ref of prioritizeImageReferences(candidateRefs)) {
    let sourceImage: EnhanceImageContext | null = null;

    if (ref.dataUrl) {
      sourceImage = ref.dataUrl;
    } else {
      const lookup = await getAttachmentLookup();
      const attachment =
        (ref.attachmentId ? lookup?.byId.get(ref.attachmentId) : undefined) ??
        (ref.filename ? lookup?.byId.get(ref.filename) : undefined) ??
        (ref.filename ? lookup?.byFilename.get(ref.filename) : undefined);

      if (!attachment || seen.has(attachment.attachmentId)) {
        continue;
      }

      seen.add(attachment.attachmentId);
      sourceImage = await readImageAttachment(sessionId, attachment);
    }

    if (!sourceImage) {
      continue;
    }

    const budgetedImage = await prepareImageForBudget(
      sourceImage,
      totalImageBytes,
    );
    if (!budgetedImage) {
      continue;
    }

    const imageBytes = getBase64ByteLength(budgetedImage.base64);
    images.push({ index: ref.index, image: budgetedImage });
    totalImageBytes += imageBytes;
    if (images.length >= MAX_IMAGE_COUNT) {
      return sortImageContexts(images);
    }
  }

  return sortImageContexts(images);
}

async function prepareImageForBudget(
  image: EnhanceImageContext,
  currentTotalBytes: number,
): Promise<EnhanceImageContext | null> {
  const targetBytes = Math.min(
    MAX_IMAGE_BYTES,
    MAX_TOTAL_IMAGE_BYTES - currentTotalBytes,
  );
  if (targetBytes <= 0) {
    return null;
  }

  const imageBytes = getBase64ByteLength(image.base64);
  if (imageBytes <= targetBytes) {
    return image;
  }

  return compressImageContext(image, targetBytes);
}

async function compressImageContext(
  image: EnhanceImageContext,
  targetBytes: number,
): Promise<EnhanceImageContext | null> {
  if (
    typeof createImageBitmap !== "function" ||
    typeof fetch !== "function" ||
    typeof document === "undefined"
  ) {
    return null;
  }

  let bitmap: ImageBitmap;
  try {
    const response = await fetch(toDataUrl(image));
    bitmap = await createImageBitmap(await response.blob());
  } catch {
    return null;
  }

  try {
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      return null;
    }

    let maxEdge = MAX_COMPRESSED_IMAGE_EDGE;
    while (maxEdge >= MIN_COMPRESSED_IMAGE_EDGE) {
      const scale = Math.min(
        1,
        maxEdge / Math.max(bitmap.width, bitmap.height),
      );
      canvas.width = Math.max(1, Math.round(bitmap.width * scale));
      canvas.height = Math.max(1, Math.round(bitmap.height * scale));
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.drawImage(bitmap, 0, 0, canvas.width, canvas.height);

      for (const quality of COMPRESSED_IMAGE_QUALITY_STEPS) {
        const compressed = parseGeneratedImageDataUrl(
          canvas.toDataURL(COMPRESSED_IMAGE_MIME_TYPE, quality),
          image.filename,
        );
        if (
          compressed &&
          getBase64ByteLength(compressed.base64) <= targetBytes
        ) {
          return compressed;
        }
      }

      maxEdge = Math.floor(maxEdge * 0.75);
    }
  } finally {
    bitmap.close?.();
  }

  return null;
}

function getCandidateImageReferences(
  references: ImageReference[],
): IndexedImageReference[] {
  return references.flatMap((ref, index) =>
    ref.dataUrl || ref.attachmentId || ref.filename ? [{ ...ref, index }] : [],
  );
}

function prioritizeImageReferences(
  references: IndexedImageReference[],
): IndexedImageReference[] {
  if (references.length <= MAX_IMAGE_COUNT) {
    return references;
  }

  const selectedIndexes = new Set<number>();
  for (let i = 0; i < MAX_IMAGE_COUNT; i++) {
    selectedIndexes.add(
      Math.round((i * (references.length - 1)) / (MAX_IMAGE_COUNT - 1)),
    );
  }

  const sampled = [...selectedIndexes].map((index) => references[index]);
  const remaining = references.filter(
    (_, index) => !selectedIndexes.has(index),
  );
  return [...sampled, ...remaining];
}

function sortImageContexts(
  images: Array<{ index: number; image: EnhanceImageContext }>,
): EnhanceImageContext[] {
  return images.sort((a, b) => a.index - b.index).map(({ image }) => image);
}

function toDataUrl(image: EnhanceImageContext): string {
  return `data:${image.mimeType};base64,${image.base64}`;
}

function parseGeneratedImageDataUrl(
  src: string,
  filename: string | undefined,
): EnhanceImageContext | null {
  const match = src.match(
    /^data:(image\/(?:gif|jpe?g|png|webp));base64,(.+)$/i,
  );
  if (!match) {
    return null;
  }

  return {
    base64: match[2],
    mimeType: match[1].toLowerCase() === "image/jpg" ? "image/jpeg" : match[1],
    filename,
  };
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

  if (readResult.data.length > MAX_SOURCE_IMAGE_BYTES) {
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
  if (getBase64ByteLength(base64) > MAX_SOURCE_IMAGE_BYTES) {
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
