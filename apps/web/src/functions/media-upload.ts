import { getSupabaseBrowserClient } from "@/functions/supabase";
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
  token: string;
}

async function registerUploadedMedia(params: {
  path: string;
  publicUrl: string;
  mimeType: string | null;
  size: number;
}) {
  const response = await fetch("/api/admin/media/register", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(params),
  });

  if (!response.ok) {
    const data = await response.json().catch(() => null);
    throw new Error(data?.error || "Failed to register media");
  }
}

async function requestSignedUpload(
  endpoint: string,
  body: Record<string, unknown>,
) {
  const response = await fetch(endpoint, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  const data = await response.json();

  if (!response.ok) {
    throw new Error(data.error || "Upload failed");
  }

  return data as SignedUploadData;
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

async function dataUrlToFile(
  dataUrl: string,
  filename: string,
  mimeType: string,
) {
  const response = await fetch(dataUrl);
  const blob = await response.blob();

  return new File([blob], filename, {
    type: blob.type || `image/${mimeType}`,
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
    const file = await dataUrlToFile(
      image.dataUrl,
      `image-${i + 1}.${extension}`,
      image.mimeType,
    );
    const uploadResult = await uploadBlogImageFile({ file, folder });

    nextContent = nextContent.replace(
      image.fullMatch,
      `![](${uploadResult.publicUrl})`,
    );
  }

  return nextContent;
}
