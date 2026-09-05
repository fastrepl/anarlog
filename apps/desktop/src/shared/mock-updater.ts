type DownloadEvent =
  | { event: "Started"; data: { contentLength?: number } }
  | { event: "Progress"; data: { chunkLength: number } }
  | { event: "Finished" };

interface Update {
  available: boolean;
  currentVersion: string;
  version: string;
  date?: string;
  body?: string;
  download: (onEvent?: (progress: DownloadEvent) => void) => Promise<void>;
  install: () => Promise<void>;
  close: () => Promise<void>;
}

export const check = check_1;

export async function check_1(): Promise<Update | null> {
  await new Promise((resolve) => setTimeout(resolve, 1000));
  return null;
}
