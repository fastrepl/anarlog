import { getSupabaseBrowserClient } from "@/functions/supabase";
import { fetchAdminJson } from "@/lib/admin-auth";
import {
  extractBase64Images,
  extractSlugFromPath,
  getExtensionFromMimeType,
  getMimeTypeFromExtension,
  MEDIA_BUCKET_NAME,
  parseMediaFilename,
} from "@/lib/media";

interface SignedUploadData {
  path: string;
  publicUrl: string;
  proxyUrl: string;
  token: string;
}

async function registerUploadedMedia(params: {
  path: string;
  publicUrl: string;
  mimeType: string | null;
  size: number;
}) {
  await fetchAdminJson(
    "/api/admin/media/register",
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(params),
    },
    "Failed to register media",
  );
}

async function requestSignedUpload(
  endpoint: string,
  body: Record<string, unknown>,
) {
  return fetchAdminJson<SignedUploadData>(
    endpoint,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    },
    "Upload failed",
  );
}

async function uploadToSignedUrl(file: File, signedUpload: SignedUploadData) {
  const supabase = getSupabaseBrowserClient();
  const parsedFilename = parseMediaFilename(file.name);
  const contentType =
    file.type ||
    (parsedFilename
      ? getMimeTypeFromExtension(parsedFilename.extension)
      : undefined);
  const { error } = await supabase.storage
    .from(MEDIA_BUCKET_NAME)
    .uploadToSignedUrl(signedUpload.path, signedUpload.token, file, {
      contentType,
    });

  if (error) {
    throw new Error(error.message);
  }

  await registerUploadedMedia({
    path: signedUpload.path,
    publicUrl: signedUpload.publicUrl,
    mimeType: contentType || file.type || null,
    size: file.size,
  });

  return {
    path: signedUpload.path,
    publicUrl: signedUpload.publicUrl,
    proxyUrl: signedUpload.proxyUrl,
  };
}

export async function uploadBlogImageFile(params: {
  file: File;
  folder?: string;
}) {
  const signedUpload = await requestSignedUpload(
    "/api/admin/blog/upload-image",
    {
      filename: params.file.name,
      folder: params.folder,
    },
  );

  return uploadToSignedUrl(params.file, signedUpload);
}

export async function uploadMediaLibraryFile(params: {
  file: File;
  folder?: string;
  path?: string;
  upsert?: boolean;
}) {
  const signedUpload = await requestSignedUpload("/api/admin/media/upload", {
    filename: params.file.name,
    folder: params.folder,
    path: params.path,
    upsert: params.upsert,
  });

  return uploadToSignedUrl(params.file, signedUpload);
}

function decodeBase64ToBytes(base64Data: string) {
  const normalizedBase64 = base64Data.replace(/\s+/g, "");
  const binary = atob(normalizedBase64);
  const bytes = new Uint8Array(binary.length);

  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }

  return bytes;
}

function base64ImageToFile(
  base64Data: string,
  filename: string,
  mimeType: string,
) {
  return new File([decodeBase64ToBytes(base64Data)], filename, {
    type: getMimeTypeFromExtension(getExtensionFromMimeType(mimeType)),
  });
}

export async function uploadInlineMarkdownImages(params: {
  content: string;
  path: string;
}) {
  const base64Images = extractBase64Images(params.content);
  if (base64Images.length === 0) {
    return params.content;
  }

  const folder = `articles/${extractSlugFromPath(params.path)}`;
  let nextContent = params.content;

  for (let i = 0; i < base64Images.length; i++) {
    const image = base64Images[i];
    const extension = getExtensionFromMimeType(image.mimeType);
    let file: File;

    try {
      file = base64ImageToFile(
        image.base64Data,
        `image-${i + 1}.${extension}`,
        image.mimeType,
      );
    } catch (error) {
      throw new Error(
        `Failed to process pasted inline image ${i + 1}: ${
          error instanceof Error ? error.message : "Invalid image data"
        }`,
      );
    }

    const uploadResult = await uploadBlogImageFile({ file, folder });

    nextContent = nextContent.replace(
      image.fullMatch,
      `![](${uploadResult.proxyUrl})`,
    );
  }

  return nextContent;
}
