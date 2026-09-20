import { describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  listDirectoryEntries: vi.fn(),
  readTextFiles: vi.fn(),
}));

vi.mock("@anlg/plugin-importer", () => ({
  commands: {
    listDirectoryEntries: mocks.listDirectoryEntries,
    readTextFiles: mocks.readTextFiles,
  },
}));

import {
  discoverFirefliesExportBundleDirs,
  loadFirefliesExportBundle,
} from "./fireflies-export";

function dirEntry(name: string, path: string, isDir = false) {
  return { name, path, isDir };
}

function mockReadTextFiles(contentByPath: Record<string, string>) {
  mocks.readTextFiles.mockImplementation(async (paths: string[]) => ({
    status: "ok",
    data: paths.map((path) => ({
      path,
      name: path.split("/").pop() ?? path,
      content: contentByPath[path] ?? "",
    })),
  }));
}

describe("discoverFirefliesExportBundleDirs", () => {
  it("treats the root as a single bundle when it directly contains transcript.json", async () => {
    mocks.listDirectoryEntries.mockResolvedValue({
      status: "ok",
      data: [dirEntry("transcript.json", "/root/transcript.json")],
    });

    const dirs = await discoverFirefliesExportBundleDirs("/root");

    expect(dirs).toEqual(["/root"]);
  });

  it("discovers only the subfolders that contain transcript.json", async () => {
    mocks.listDirectoryEntries.mockImplementation(async (path: string) => {
      if (path === "/root") {
        return {
          status: "ok",
          data: [
            dirEntry("call-a", "/root/call-a", true),
            dirEntry("call-b", "/root/call-b", true),
            dirEntry(".DS_Store", "/root/.DS_Store"),
          ],
        };
      }
      if (path === "/root/call-a") {
        return {
          status: "ok",
          data: [dirEntry("transcript.json", "/root/call-a/transcript.json")],
        };
      }
      return {
        status: "ok",
        data: [dirEntry("notes.txt", "/root/call-b/notes.txt")],
      };
    });

    const dirs = await discoverFirefliesExportBundleDirs("/root");

    expect(dirs).toEqual(["/root/call-a"]);
  });
});

describe("loadFirefliesExportBundle", () => {
  it("reshapes a bundle's files into an importable meeting record", async () => {
    mocks.listDirectoryEntries.mockResolvedValue({
      status: "ok",
      data: [
        dirEntry("transcript.json", "/bundle/transcript.json"),
        dirEntry(
          "meeting-metadata-ID1.txt",
          "/bundle/meeting-metadata-ID1.txt",
        ),
        dirEntry("meeting-url-ID1.txt", "/bundle/meeting-url-ID1.txt"),
        dirEntry("meeting-summary-ID1.txt", "/bundle/meeting-summary-ID1.txt"),
        dirEntry("transcript-ID1.md", "/bundle/transcript-ID1.md"),
        dirEntry("audio.mp3", "/bundle/audio.mp3"),
      ],
    });
    mockReadTextFiles({
      "/bundle/transcript.json": JSON.stringify({
        data: [
          {
            sentence: "Le classique.",
            speaker_name: "Alexis Murat",
            time: 2.5,
            endTime: 3.1,
          },
        ],
      }),
      "/bundle/meeting-metadata-ID1.txt": JSON.stringify({
        attendees: [],
        meetingTitle: "Weekly sync",
        meetingStartTime: "2026-08-17T12:45:50.640Z",
        meetingEndTime: "2026-08-17T13:30:33.552Z",
      }),
      "/bundle/meeting-url-ID1.txt": "https://app.fireflies.ai/view/ID1\n",
      "/bundle/meeting-summary-ID1.txt": "We agreed to ship.",
    });

    const bundle = await loadFirefliesExportBundle("/bundle");

    expect(bundle?.audioPath).toBe("/bundle/audio.mp3");
    const record = JSON.parse(bundle!.file.content);
    expect(record).toMatchObject({
      id: "ID1",
      title: "Weekly sync",
      started_at: "2026-08-17T12:45:50.640Z",
      ended_at: "2026-08-17T13:30:33.552Z",
      url: "https://app.fireflies.ai/view/ID1",
      summary: "We agreed to ship.",
      transcript: [
        {
          speaker: "Alexis Murat",
          text: "Le classique.",
          start: 2.5,
          end: 3.1,
        },
      ],
    });
  });

  it("omits the summary field when Fireflies reports no summary", async () => {
    mocks.listDirectoryEntries.mockResolvedValue({
      status: "ok",
      data: [
        dirEntry("transcript.json", "/bundle/transcript.json"),
        dirEntry("meeting-summary-ID1.txt", "/bundle/meeting-summary-ID1.txt"),
      ],
    });
    mockReadTextFiles({
      "/bundle/transcript.json": JSON.stringify({ data: [] }),
      "/bundle/meeting-summary-ID1.txt": "No summary found",
    });

    const bundle = await loadFirefliesExportBundle("/bundle");

    const record = JSON.parse(bundle!.file.content);
    expect(record.summary).toBeUndefined();
  });

  it("returns null when the folder has no transcript.json", async () => {
    mocks.listDirectoryEntries.mockResolvedValue({
      status: "ok",
      data: [dirEntry("notes.txt", "/bundle/notes.txt")],
    });

    const bundle = await loadFirefliesExportBundle("/bundle");

    expect(bundle).toBeNull();
  });

  it("leaves audioPath empty when the bundle has no audio file", async () => {
    mocks.listDirectoryEntries.mockResolvedValue({
      status: "ok",
      data: [dirEntry("transcript.json", "/bundle/transcript.json")],
    });
    mockReadTextFiles({
      "/bundle/transcript.json": JSON.stringify({ data: [] }),
    });

    const bundle = await loadFirefliesExportBundle("/bundle");

    expect(bundle?.audioPath).toBe("");
  });
});
