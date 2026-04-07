export const MEDIA_BUCKET_NAME = "blog";
export const MEDIA_PROXY_BASE_PATH = "/api/assets";

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
}

export function extractBase64Images(markdown: string): Base64Image[] {
  const regex = /!\[[^\]]*\]\((data:image\/([^;]+);base64,([^)]+))\)/g;
  const images: Base64Image[] = [];
  let match;

  while ((match = regex.exec(markdown)) !== null) {
    images.push({
      fullMatch: match[0],
      mimeType: match[2],
      base64Data: match[3],
      dataUrl: match[1],
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
