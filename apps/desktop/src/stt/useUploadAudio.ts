import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback, useRef, useState } from "react";

import { uploadAudio } from "@hypr/supabase/storage";

import { useAuth } from "~/auth";
import { env } from "~/env";

export function useUploadAudio() {
  const auth = useAuth();
  const [progress, setProgress] = useState<number | null>(null);
  const abortRef = useRef<(() => void) | null>(null);

  const upload = useCallback(
    async (filePath: string): Promise<string> => {
      if (!auth.session || !env.VITE_SUPABASE_URL) {
        throw new Error("Not authenticated or Supabase not configured");
      }

      const assetUrl = convertFileSrc(filePath);
      const response = await fetch(assetUrl);
      const blob = await response.blob();

      const fileName = filePath.split("/").pop() ?? "audio";
      const contentType = blob.type || "audio/ogg";

      setProgress(0);

      const { promise, abort } = uploadAudio({
        file: blob,
        fileName,
        contentType,
        supabaseUrl: env.VITE_SUPABASE_URL,
        accessToken: auth.session.access_token,
        userId: auth.session.user.id,
        onProgress: setProgress,
      });

      abortRef.current = abort;

      try {
        const fileId = await promise;
        setProgress(null);
        return fileId;
      } catch (error) {
        setProgress(null);
        throw error;
      }
    },
    [auth.session],
  );

  const abort = useCallback(() => {
    abortRef.current?.();
    setProgress(null);
  }, []);

  return { upload, abort, progress };
}
