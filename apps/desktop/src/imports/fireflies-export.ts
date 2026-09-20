import {
  commands as importerCommands,
  type ImportDirectoryEntry,
  type ImportTextFile,
} from "@anlg/plugin-importer";

const TRANSCRIPT_FILE_NAME = "transcript.json";
const AUDIO_FILE_NAME = "audio.mp3";
const METADATA_FILE_PREFIX = "meeting-metadata-";
const URL_FILE_PREFIX = "meeting-url-";
const SUMMARY_FILE_PREFIX = "meeting-summary-";
const NO_SUMMARY_PLACEHOLDER = "No summary found";
const FIREFLIES_ID_PATTERN = /\/view\/([^/?#\s]+)/u;

export type FirefliesExportBundleFile = {
  file: ImportTextFile;
  audioPath: string;
};

type FirefliesExportMetadata = {
  attendees?: unknown[];
  meetingTitle?: string;
  meetingStartTime?: string;
  meetingEndTime?: string;
};

type FirefliesExportTranscript = {
  data?: Array<{
    sentence?: string;
    speaker_name?: string;
    time?: number;
    endTime?: number;
  }>;
};

export async function loadFirefliesExportBundles(
  rootPath: string,
): Promise<FirefliesExportBundleFile[]> {
  const bundleDirs = await discoverFirefliesExportBundleDirs(rootPath);
  const bundles = await Promise.all(
    bundleDirs.map((dir) => loadFirefliesExportBundle(dir)),
  );
  return bundles.filter(
    (bundle): bundle is FirefliesExportBundleFile => bundle !== null,
  );
}

export async function discoverFirefliesExportBundleDirs(
  rootPath: string,
): Promise<string[]> {
  const rootEntries = await listDirectory(rootPath);
  if (containsTranscript(rootEntries)) {
    return [rootPath];
  }

  const childEntries = await Promise.all(
    rootEntries
      .filter((entry) => entry.isDir)
      .map(async (entry) => ({
        entry,
        children: await listDirectory(entry.path).catch(() => []),
      })),
  );

  return childEntries
    .filter(({ children }) => containsTranscript(children))
    .map(({ entry }) => entry.path);
}

export async function loadFirefliesExportBundle(
  bundleDir: string,
): Promise<FirefliesExportBundleFile | null> {
  const entries = await listDirectory(bundleDir);
  const transcriptEntry = findEntry(
    entries,
    (name) => name === TRANSCRIPT_FILE_NAME,
  );
  if (!transcriptEntry) {
    return null;
  }

  const metadataEntry = findEntry(
    entries,
    (name) => name.startsWith(METADATA_FILE_PREFIX) && name.endsWith(".txt"),
  );
  const urlEntry = findEntry(
    entries,
    (name) => name.startsWith(URL_FILE_PREFIX) && name.endsWith(".txt"),
  );
  const summaryEntry = findEntry(
    entries,
    (name) => name.startsWith(SUMMARY_FILE_PREFIX) && name.endsWith(".txt"),
  );
  const audioEntry = findEntry(entries, (name) => name === AUDIO_FILE_NAME);

  const textPaths = [transcriptEntry, metadataEntry, urlEntry, summaryEntry]
    .filter((entry): entry is ImportDirectoryEntry => entry !== undefined)
    .map((entry) => entry.path);
  const result = await importerCommands.readTextFiles(textPaths);
  if (result.status === "error") {
    throw new Error(result.error);
  }
  const contentByPath = new Map(
    result.data.map((file) => [file.path, file.content]),
  );

  const metadataText = metadataEntry
    ? contentByPath.get(metadataEntry.path)
    : undefined;
  const urlText = urlEntry ? contentByPath.get(urlEntry.path) : undefined;
  const summaryText = summaryEntry
    ? contentByPath.get(summaryEntry.path)
    : undefined;
  const transcriptText = contentByPath.get(transcriptEntry.path) ?? "";

  const metadata: FirefliesExportMetadata = metadataText
    ? JSON.parse(metadataText)
    : {};
  const transcript: FirefliesExportTranscript = transcriptText
    ? JSON.parse(transcriptText)
    : {};

  const externalId = extractFirefliesId(urlText ?? "") || transcriptEntry.path;
  const summary = summaryText?.trim() ?? "";

  const record = {
    id: externalId,
    title: metadata.meetingTitle ?? "",
    started_at: metadata.meetingStartTime ?? "",
    ended_at: metadata.meetingEndTime ?? "",
    url: urlText?.trim() ?? "",
    ...(summary && summary !== NO_SUMMARY_PLACEHOLDER ? { summary } : {}),
    attendees: metadata.attendees ?? [],
    transcript: (transcript.data ?? []).map((sentence) => ({
      speaker: sentence.speaker_name ?? "",
      text: sentence.sentence ?? "",
      start: sentence.time ?? 0,
      end: sentence.endTime ?? 0,
    })),
  };

  return {
    file: {
      path: transcriptEntry.path,
      name: `${sanitizeFileName(externalId)}.json`,
      content: JSON.stringify(record),
    },
    audioPath: audioEntry?.path ?? "",
  };
}

function containsTranscript(entries: ImportDirectoryEntry[]) {
  return entries.some(
    (entry) => !entry.isDir && entry.name === TRANSCRIPT_FILE_NAME,
  );
}

function findEntry(
  entries: ImportDirectoryEntry[],
  predicate: (name: string) => boolean,
) {
  return entries.find((entry) => !entry.isDir && predicate(entry.name));
}

async function listDirectory(path: string) {
  const result = await importerCommands.listDirectoryEntries(path);
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

function extractFirefliesId(url: string) {
  return url.trim().match(FIREFLIES_ID_PATTERN)?.[1] ?? "";
}

function sanitizeFileName(value: string) {
  return value.replace(/[^a-zA-Z0-9-]+/gu, "-") || "meeting";
}
