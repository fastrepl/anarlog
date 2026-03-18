import { invoke } from "@tauri-apps/api/core";

const IMAGE_MIME_TYPES: Record<string, string> = {
  avif: "image/avif",
  gif: "image/gif",
  jpeg: "image/jpeg",
  jpg: "image/jpeg",
  png: "image/png",
  svg: "image/svg+xml",
  webp: "image/webp",
};

function isTauriAssetUrl(src: string) {
  try {
    const url = new URL(src);

    return (
      (url.protocol === "asset:" && url.hostname === "localhost") ||
      ((url.protocol === "http:" || url.protocol === "https:") &&
        url.hostname === "asset.localhost")
    );
  } catch {
    return false;
  }
}

function getAssetPath(src: string) {
  try {
    const url = new URL(src);
    return decodeURIComponent(url.pathname.replace(/^\/+/, "/"));
  } catch {
    return null;
  }
}

function guessImageMimeType(src: string) {
  const assetPath = getAssetPath(src) ?? src;
  const extension = assetPath.split(".").pop()?.toLowerCase();

  return extension ? (IMAGE_MIME_TYPES[extension] ?? null) : null;
}

function blobToDataUrl(blob: Blob) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();

    reader.onload = () => {
      if (typeof reader.result === "string") {
        resolve(reader.result);
        return;
      }

      reject(new Error("Failed to read image as data URL"));
    };
    reader.onerror = () =>
      reject(reader.error ?? new Error("Failed to read image as data URL"));
    reader.readAsDataURL(blob);
  });
}

export async function resolveClipboardImageSrc(src: string) {
  if (!isTauriAssetUrl(src)) {
    return null;
  }

  try {
    const response = await fetch(src);
    if (!response.ok) {
      return null;
    }

    const blob = await response.blob();
    const mimeType = blob.type || guessImageMimeType(src);
    const typedBlob = mimeType
      ? new Blob([await blob.arrayBuffer()], { type: mimeType })
      : blob;

    return blobToDataUrl(typedBlob);
  } catch (error) {
    console.error("Failed to resolve clipboard image source:", error);
    return null;
  }
}

export async function writeClipboardHtml(html: string, altText: string) {
  await invoke("plugin:clipboard-manager|write_html", {
    html,
    altText,
    alt_text: altText,
  });
}
