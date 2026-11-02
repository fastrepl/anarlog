import { Directory, File, Paths } from "expo-file-system";

import {
  localWhisperModelInfo,
  transcribeLocalWhisper,
  verifyLocalWhisperModel,
} from "@anlg/mobile-bridge";

export type LocalListenParams = {
  languages: string[];
  keywords: string[];
};

export type LocalWhisperModelInfo = {
  model: string;
  url: string;
  uri: string;
  size_bytes: number;
};

export type LocalWhisperWord = {
  text: string;
  speaker: { type: "unassigned"; value: { index: number } } | null;
  confidence: number | null;
  start_ms: number | null;
  end_ms: number | null;
};

export function getLocalWhisperModelInfo(): LocalWhisperModelInfo {
  const modelsDirectory = new Directory(Paths.document, "models");
  return JSON.parse(
    localWhisperModelInfo(modelsDirectory.uri),
  ) as LocalWhisperModelInfo;
}

export async function isLocalWhisperModelReady(
  info = getLocalWhisperModelInfo(),
): Promise<boolean> {
  const file = new File(info.uri);
  return file.exists && verifyLocalWhisperModel(info.uri);
}

export async function downloadLocalWhisperModel(
  onProgress: (progress: number) => void,
  onTask: (task: ReturnType<typeof File.createDownloadTask>) => void,
): Promise<void> {
  const info = getLocalWhisperModelInfo();
  const modelDirectory = new Directory(Paths.document, "models", "stt");
  modelDirectory.create({ intermediates: true, idempotent: true });
  const destination = new File(info.uri);
  if (destination.exists) destination.delete();

  const task = File.createDownloadTask(info.url, destination, {
    onProgress: ({ bytesWritten, totalBytes }) => {
      onProgress(totalBytes > 0 ? Math.min(bytesWritten / totalBytes, 1) : 0);
    },
  });
  onTask(task);
  try {
    const downloaded = await task.downloadAsync();
    if (!downloaded) throw new Error("The model download did not finish.");
    if (!(await verifyLocalWhisperModel(info.uri))) {
      destination.delete();
      throw new Error("The downloaded model failed its integrity check.");
    }
  } finally {
    task.release();
  }
}

export async function transcribeLocally(
  audioUri: string,
  params: LocalListenParams,
): Promise<LocalWhisperWord[]> {
  const info = getLocalWhisperModelInfo();
  if (!new File(info.uri).exists) {
    throw Object.assign(
      new Error("Download the on-device Whisper model in Settings first."),
      { code: "local_model_missing" },
    );
  }
  return JSON.parse(
    await transcribeLocalWhisper(
      JSON.stringify({ audio_uri: audioUri, model_uri: info.uri, params }),
    ),
  ) as LocalWhisperWord[];
}

export function discardLocalWhisperCheckpoint(audioUri: string): void {
  const audioFile = new File(audioUri);
  const checkpoint = new File(
    audioFile.parentDirectory,
    `${audioFile.name}.anlg-transcription-checkpoint.jsonl`,
  );
  if (checkpoint.exists) checkpoint.delete();
}
