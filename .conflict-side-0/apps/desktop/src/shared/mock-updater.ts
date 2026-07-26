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

export async function check_2(): Promise<Update | null> {
  await new Promise((resolve) => setTimeout(resolve, 1000));

  return {
    available: true,
    currentVersion: "0.1.0",
    version: "0.2.0",
    date: new Date().toISOString(),
    body: "## What's New\n\n- New feature: Dark mode\n- Bug fixes and improvements",

    download: async (onEvent) => {
      const totalSize = 50 * 1024 * 1024;
      const chunkSize = 512 * 1024;
      const chunks = Math.floor(totalSize / chunkSize);

      onEvent?.({ event: "Started", data: { contentLength: totalSize } });

      for (let i = 0; i < chunks; i++) {
        await new Promise((resolve) => setTimeout(resolve, 50));
        onEvent?.({
          event: "Progress",
          data: { chunkLength: chunkSize },
        });
      }

      onEvent?.({ event: "Finished" });
    },

    install: async () => {
      await new Promise((resolve) => setTimeout(resolve, 1000));
    },

    close: async () => {},
  };
}

export async function check_3(): Promise<Update | null> {
  await new Promise((resolve) => setTimeout(resolve, 500));

  return {
    available: true,
    currentVersion: "0.1.0",
    version: "0.1.1",
    date: new Date().toISOString(),
    body: "## Minor Update\n\n- Critical security patch",

    download: async (onEvent) => {
      const totalSize = 10 * 1024 * 1024;
      onEvent?.({ event: "Started", data: { contentLength: totalSize } });
      await new Promise((resolve) => setTimeout(resolve, 200));
      onEvent?.({ event: "Progress", data: { chunkLength: totalSize } });
      onEvent?.({ event: "Finished" });
    },

    install: async () => {
      await new Promise((resolve) => setTimeout(resolve, 500));
    },

    close: async () => {},
  };
}
