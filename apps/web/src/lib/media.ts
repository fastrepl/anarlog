export const MEDIA_BUCKET_NAME = "blog";
export const MEDIA_PROXY_BASE_PATH = "/api/assets";
const MEDIA_PUBLIC_BUCKET_PATH = "/storage/v1/object/public/blog/";

const MEDIA_MIME_TYPES: Record<string, string> = {
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  png: "image/png",
  gif: "image/gif",
  svg: "image/svg+xml",
  webp: "image/webp",
  avif: "image/avif",
  mp4: "video/mp4",
  webm: "video/webm",
  mov: "video/quicktime",
};

const MIME_TYPE_EXTENSIONS: Record<string, string> = {
  jpeg: "jpg",
  jpg: "jpg",
  png: "png",
  gif: "gif",
  webp: "webp",
  svg: "svg",
  "svg+xml": "svg",
  avif: "avif",
};

export interface Base64Image {
  fullMatch: string;
  mimeType: string;
  base64Data: string;
  dataUrl: string;
  altText: string;
  title: string | null;
}

const BASE64_IMAGE_MARKDOWN_REGEX =
  /!\[(?<altText>[^\]]*)\]\((?<dataUrl>data:image\/[^\s)]+)(?:\s+(?<title>"[^"]*"|'[^']*'))?\)/g;
const BASE64_IMAGE_DATA_URL_REGEX =
  /^data:(image\/[^;,]+)(?:;[^,]+)*;base64,(.+)$/i;

export function normalizeBase64Data(base64Data: string): string {
  let normalized = base64Data.replace(/\s+/g, "");

  if (normalized.includes("%")) {
    try {
      normalized = decodeURIComponent(normalized);
    } catch {
      // Keep the original string when percent decoding fails.
    }
  }

  normalized = normalized.replace(/-/g, "+").replace(/_/g, "/");

  const padding = normalized.length % 4;
  if (padding !== 0) {
    normalized = normalized.padEnd(normalized.length + (4 - padding), "=");
  }

  return normalized;
}

function parseBase64ImageDataUrl(dataUrl: string) {
  const match = dataUrl.match(BASE64_IMAGE_DATA_URL_REGEX);
  if (!match) {
    return null;
  }

  const [, mimeType, base64Data] = match;

  return {
    mimeType: mimeType.split("/")[1].toLowerCase(),
    base64Data: normalizeBase64Data(base64Data),
  };
}

export function extractBase64Images(markdown: string): Base64Image[] {
  const images: Base64Image[] = [];
  let match: RegExpExecArray | null;

  while ((match = BASE64_IMAGE_MARKDOWN_REGEX.exec(markdown)) !== null) {
    const groups = match.groups;
    if (!groups?.dataUrl) {
      continue;
    }

    const parsed = parseBase64ImageDataUrl(groups.dataUrl);
    if (!parsed) {
      continue;
    }

    const rawTitle = groups.title;
    images.push({
      fullMatch: match[0],
      mimeType: parsed.mimeType,
      base64Data: parsed.base64Data,
      dataUrl: groups.dataUrl,
      altText: groups.altText || "",
      title: rawTitle ? rawTitle.slice(1, -1) : null,
    });
  }

  return images;
}

export function extractSlugFromPath(path: string): string {
  const filename = path.split("/").pop() || "";
  return filename.replace(/\.mdx$/, "");
}

export function getExtensionFromMimeType(mimeType: string): string {
  return MIME_TYPE_EXTENSIONS[mimeType] || "png";
}

export function getMimeTypeFromExtension(extension: string): string {
  return MEDIA_MIME_TYPES[extension] || "application/octet-stream";
}

export function parseMediaFilename(filename: string) {
  const parts = filename.split(".");
  const extension = parts.pop()?.toLowerCase();
  const baseName = parts.join(".").replace(/[^a-zA-Z0-9.-]/g, "-") || "file";

  if (!extension || !(extension in MEDIA_MIME_TYPES)) {
    return null;
  }

  return {
    extension,
    baseName,
    filename: `${baseName}.${extension}`,
  };
}

export function getMediaProxyUrl(path: string): string {
  const normalizedPath = path.split("/").filter(Boolean);

  return `${MEDIA_PROXY_BASE_PATH}/blog/${normalizedPath
    .map((segment) => encodeURIComponent(segment))
    .join("/")}`;
}

function decodeMediaPath(rawPath: string): string | null {
  const trimmedPath = rawPath.split("?")[0]?.split("#")[0] || "";
  if (!trimmedPath) return null;

  try {
    return trimmedPath
      .split("/")
      .filter(Boolean)
      .map((segment) => decodeURIComponent(segment))
      .join("/");
  } catch {
    return null;
  }
}

function extractMediaPathFromPublicUrl(url: string): string | null {
  try {
    const parsedUrl = new URL(url);
    const bucketIndex = parsedUrl.pathname.indexOf(MEDIA_PUBLIC_BUCKET_PATH);
    if (bucketIndex === -1) {
      return null;
    }

    return decodeMediaPath(
      parsedUrl.pathname.slice(bucketIndex + MEDIA_PUBLIC_BUCKET_PATH.length),
    );
  } catch {
    return null;
  }
}

export function extractManagedMediaPaths(text: string): string[] {
  const mediaPaths = new Set<string>();
  const proxyRegex = /\/api\/assets\/blog\/[^\s)"']+/g;
  const publicRegex =
    /https?:\/\/[^\s)"']+\/storage\/v1\/object\/public\/blog\/[^\s)"']+/g;

  for (const match of text.match(proxyRegex) || []) {
    const path = decodeMediaPath(
      match.slice(`${MEDIA_PROXY_BASE_PATH}/blog/`.length),
    );
    if (path) {
      mediaPaths.add(path);
    }
  }

  for (const match of text.match(publicRegex) || []) {
    const path = extractMediaPathFromPublicUrl(match);
    if (path) {
      mediaPaths.add(path);
    }
  }

  return [...mediaPaths];
}
