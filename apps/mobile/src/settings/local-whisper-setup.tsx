import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { File } from "expo-file-system";
import { useRef, useState } from "react";

import {
  downloadLocalWhisperModel,
  getLocalWhisperModelInfo,
  isLocalWhisperModelReady,
} from "@/data/local-whisper";
import { Button, Text } from "@/settings/fields";

export function LocalWhisperSetup() {
  const queryClient = useQueryClient();
  const [progress, setProgress] = useState(0);
  const task = useRef<ReturnType<typeof File.createDownloadTask> | null>(null);
  const info = getLocalWhisperModelInfo();
  const model = useQuery({
    queryKey: ["local-whisper-model"],
    queryFn: () => isLocalWhisperModelReady(info),
  });
  const download = useMutation({
    mutationFn: () =>
      downloadLocalWhisperModel(setProgress, (nextTask) => {
        task.current = nextTask;
      }),
    onSuccess: async () => {
      setProgress(0);
      task.current = null;
      await queryClient.invalidateQueries({
        queryKey: ["local-whisper-model"],
      });
    },
    onError: async () => {
      task.current = null;
      await queryClient.invalidateQueries({
        queryKey: ["local-whisper-model"],
      });
    },
  });

  return (
    <>
      <Text>{`Downloads ${info.model} (${(info.size_bytes / 1_000_000).toFixed(0)} MB) to this device. Audio and transcription stay local. A summary provider receives transcript text only when you tap Generate summary.`}</Text>
      {download.isPending ? (
        <>
          <Text>{`Downloading… ${Math.round(progress * 100)}%`}</Text>
          <Button
            label="Cancel download"
            onPress={() => task.current?.cancel()}
          />
        </>
      ) : model.data ? (
        <Text>Model ready for offline transcription.</Text>
      ) : (
        <Button
          label={model.isPending ? "Checking model…" : "Download model"}
          onPress={() => download.mutate()}
        />
      )}
      {download.error instanceof Error && <Text>{download.error.message}</Text>}
    </>
  );
}
