import { getDocumentAsync } from "expo-document-picker";
import { Directory, File, Paths } from "expo-file-system";
import * as Sharing from "expo-sharing";

import { catalogSessionAudio } from "@/data/audio-catalog";
import { docToPlainText, stripMarkdownTitle } from "@/data/note-doc";
import { createSession, deleteSession, saveSessionNote } from "@/data/session";
import { execute, executeTransaction } from "@/db";
import { id, nowIso } from "@/lib/ids";

import {
  readMeetingArchive,
  writeMeetingArchive,
  type MeetingArchiveAudio,
} from "./meeting-package-archive";
import { MAX_TRANSCRIPTION_AUDIO_BYTES } from "./transcription-response";

const FORMAT = "anarlog.meeting";
const VERSION = 1;
const MAX_MANIFEST_BYTES = 32 * 1024 * 1024;
const MAX_PACKAGE_BYTES =
  MAX_TRANSCRIPTION_AUDIO_BYTES + MAX_MANIFEST_BYTES + 1_000_000;

type PackageTranscript = {
  id: string;
  source: string;
  provider: string;
  model: string;
  language: string;
  started_at_ms: number;
  ended_at_ms: number | null;
  memo: string;
  words_json: string;
  speaker_hints_json: string;
  metadata_json?: string;
};

type MeetingPackage = {
  format: typeof FORMAT;
  version: typeof VERSION;
  exported_at: string;
  meeting: {
    title: string;
    created_at: string;
    note: string;
    speaker_context: unknown | null;
    summaries: Array<{
      title: string;
      body: string;
      body_format: string;
      generation_metadata_json: string;
    }>;
  };
  transcripts: PackageTranscript[];
  audio: {
    content_type: string;
    filename: string;
    sha256: string;
    size_bytes: number;
  } | null;
};

export async function exportMeetingPackage(
  sessionId: string,
  includeAudio: boolean,
): Promise<void> {
  const [meetings, summaries, transcripts] = await Promise.all([
    execute<{ title: string; created_at: string; metadata_json: string }>(
      "SELECT title, created_at, metadata_json FROM sessions WHERE id = ? AND deleted_at IS NULL",
      [sessionId],
    ),
    execute<MeetingPackage["meeting"]["summaries"][number]>(
      `SELECT title, body, body_format, generation_metadata_json
       FROM session_documents WHERE session_id = ?
         AND kind IN ('summary', 'template_output') AND deleted_at IS NULL
       ORDER BY sort_order, created_at, id`,
      [sessionId],
    ),
    execute<PackageTranscript>(
      `SELECT id, source, provider, model, language, started_at_ms,
         ended_at_ms, memo, words_json, speaker_hints_json, metadata_json
       FROM transcripts WHERE session_id = ? AND deleted_at IS NULL
       ORDER BY started_at_ms, created_at, id`,
      [sessionId],
    ),
  ]);
  const meeting = meetings[0];
  if (!meeting) throw new Error("This meeting is no longer available.");
  const noteRows = await execute<{ body: string; body_format: string }>(
    `SELECT body, body_format FROM session_documents
     WHERE session_id = ? AND kind = 'note' AND deleted_at IS NULL
     ORDER BY CASE WHEN id = session_id THEN 0 ELSE 1 END, created_at, id LIMIT 1`,
    [sessionId],
  );
  const note = noteRows[0];
  const metadata = parseObject(meeting.metadata_json);
  const audioRows = includeAudio
    ? await execute<{
        filename: string;
        content_type: string;
        size_bytes: number;
        sha256: string;
        local_relative_path: string;
      }>(
        `SELECT attachment.filename, attachment.content_type,
           attachment.size_bytes, attachment.sha256, local_state.relative_path AS local_relative_path
         FROM session_attachments AS attachment
         JOIN attachment_local_state AS local_state
           ON local_state.attachment_id = attachment.id
          AND local_state.availability = 'present'
         WHERE attachment.session_id = ? AND attachment.source_type = 'session_audio'
           AND attachment.deleted_at IS NULL LIMIT 1`,
        [sessionId],
      )
    : [];
  const audioRow = audioRows[0];
  if (includeAudio && !audioRow) {
    throw new Error(
      "Download the recording to this device before exporting it.",
    );
  }
  if (audioRow && audioRow.size_bytes > MAX_TRANSCRIPTION_AUDIO_BYTES) {
    throw new Error(
      "The recording is too large to include in a meeting package.",
    );
  }
  const audioFile = audioRow
    ? new File(
        Paths.document,
        "sessions",
        sessionId,
        audioRow.local_relative_path,
      )
    : null;
  if (audioFile && !audioFile.exists) {
    throw new Error("The local recording file is missing.");
  }
  const audioDescriptor: MeetingArchiveAudio | undefined =
    audioRow && audioFile
      ? {
          contentType: safeAudioContentType(audioRow.content_type),
          filename: safeAudioFilename(audioRow.filename),
          sha256: audioRow.sha256,
          sizeBytes: audioRow.size_bytes,
        }
      : undefined;
  const payload: MeetingPackage = {
    format: FORMAT,
    version: VERSION,
    exported_at: nowIso(),
    meeting: {
      title: meeting.title,
      created_at: meeting.created_at,
      note: note ? noteText(note.body, note.body_format) : "",
      speaker_context: metadata.speaker_context ?? null,
      summaries,
    },
    transcripts,
    audio:
      audioRow && audioDescriptor
        ? {
            content_type: audioDescriptor.contentType,
            filename: audioDescriptor.filename,
            sha256: audioDescriptor.sha256,
            size_bytes: audioDescriptor.sizeBytes,
          }
        : null,
  };
  const serialized = JSON.stringify(payload);
  if (new TextEncoder().encode(serialized).byteLength > MAX_MANIFEST_BYTES) {
    throw new Error("This meeting is too large to export as a text package.");
  }
  const directory = new Directory(Paths.cache, "meeting-exports");
  directory.create({ intermediates: true, idempotent: true });
  const file = new File(directory, `${safeName(meeting.title)}.anlgmeeting`);
  try {
    await writeMeetingArchive(
      file,
      serialized,
      audioFile && audioDescriptor
        ? { file: audioFile, descriptor: audioDescriptor }
        : undefined,
    );
    if (!(await Sharing.isAvailableAsync())) {
      throw new Error("Sharing is not available on this device.");
    }
    await Sharing.shareAsync(file.uri, {
      dialogTitle: `Export ${meeting.title || "meeting"}`,
      mimeType: "application/vnd.anarlog.meeting+zip",
      UTI: "public.zip-archive",
    });
  } finally {
    if (file.exists) file.delete();
  }
}

export async function importMeetingPackage(
  ownerUserId?: string,
): Promise<string | null> {
  const picked = await getDocumentAsync({
    type: [
      "application/zip",
      "application/vnd.anarlog.meeting+zip",
      ".anlgmeeting",
    ],
    multiple: false,
    copyToCacheDirectory: true,
  });
  if (picked.canceled) return null;
  const file = new File(picked.assets[0].uri);
  const archive = await readMeetingArchive(file, {
    maxArchiveBytes: MAX_PACKAGE_BYTES,
    maxManifestBytes: MAX_MANIFEST_BYTES,
    maxAudioBytes: MAX_TRANSCRIPTION_AUDIO_BYTES,
  });
  let payload: MeetingPackage;
  try {
    payload = parseMeetingPackage(archive.manifest);
    if (Boolean(payload.audio) !== Boolean(archive.audio)) {
      throw new Error("The package is missing its declared audio file.");
    }
  } catch (error) {
    if (archive.audio?.file.exists) archive.audio.file.delete();
    throw error;
  }
  let sessionId: string;
  try {
    sessionId = await createSession({
      ownerUserId,
      title: payload.meeting.title,
      createdAt: payload.meeting.created_at,
      entryPoint: "voice_memo_import",
      trackCreated: false,
    });
  } catch (error) {
    if (archive.audio?.file.exists) archive.audio.file.delete();
    throw error;
  }
  let importedAudioFile: File | null = null;
  try {
    await saveSessionNote(sessionId, {
      title: payload.meeting.title,
      bodyText: payload.meeting.note,
      bodyFormat: "markdown",
    });
    const now = nowIso();
    const audioAttachmentId = payload.audio ? `session-audio:${sessionId}` : "";
    const transcriptIdMap = new Map(
      payload.transcripts.map((transcript) => [transcript.id, id()]),
    );
    const statements = [
      ...(payload.meeting.speaker_context === null
        ? []
        : [
            {
              sql: `UPDATE sessions SET metadata_json = json_set(
                CASE WHEN json_valid(metadata_json) THEN metadata_json ELSE '{}' END,
                '$.speaker_context', json(?)
              ), updated_at = ? WHERE id = ? AND deleted_at IS NULL`,
              params: [
                JSON.stringify(payload.meeting.speaker_context),
                now,
                sessionId,
              ],
            },
          ]),
      ...payload.transcripts.map((transcript) => ({
        sql: `INSERT INTO transcripts (
          id, workspace_id, owner_user_id, session_id, source, provider,
          model, language, started_at_ms, ended_at_ms, audio_attachment_id,
          memo, words_json, speaker_hints_json, metadata_json, created_at,
          updated_at, deleted_at
        ) SELECT ?, workspace_id, owner_user_id, id, ?, ?, ?, ?, ?, ?, ?,
          ?, ?, ?, ?, ?, ?, NULL FROM sessions
          WHERE id = ? AND deleted_at IS NULL`,
        params: [
          transcriptIdMap.get(transcript.id)!,
          transcript.source,
          transcript.provider,
          transcript.model,
          transcript.language,
          transcript.started_at_ms,
          transcript.ended_at_ms,
          audioAttachmentId,
          transcript.memo,
          transcript.words_json,
          transcript.speaker_hints_json,
          transcript.metadata_json ?? "{}",
          now,
          now,
          sessionId,
        ],
      })),
      ...payload.meeting.summaries.map((summary) => {
        let body = summary.body;
        for (const [oldId, newId] of transcriptIdMap) {
          body = body.replaceAll(
            `anarlog-evidence://${encodeURIComponent(oldId)}%3A`,
            `anarlog-evidence://${encodeURIComponent(newId)}%3A`,
          );
        }
        return {
          sql: `INSERT INTO session_documents (
            id, workspace_id, session_id, kind, title, body_format, body,
            generation_metadata_json, created_at, updated_at, deleted_at
          ) SELECT ?, workspace_id, id, 'summary', ?, ?, ?, ?, ?, ?, NULL
            FROM sessions WHERE id = ? AND deleted_at IS NULL`,
          params: [
            id(),
            summary.title,
            summary.body_format,
            body,
            summary.generation_metadata_json,
            now,
            now,
            sessionId,
          ],
        };
      }),
    ];
    await executeTransaction(statements);
    if (archive.audio) {
      const filename = safeAudioFilename(payload.audio!.filename);
      const directory = new Directory(Paths.document, "sessions", sessionId);
      directory.create({ intermediates: true, idempotent: true });
      const destination = new File(directory, filename);
      importedAudioFile = destination;
      await archive.audio.file.copy(destination);
      await catalogSessionAudio(sessionId, {
        source: "import",
        filename,
        contentType: payload.audio!.content_type,
      });
      if (payload.transcripts.length > 0) {
        const now = nowIso();
        const captureStartedAtMs = Math.min(
          ...payload.transcripts.map((transcript) => transcript.started_at_ms),
        );
        await executeTransaction([
          {
            sql: `UPDATE session_attachments SET metadata_json = json_set(
              CASE WHEN json_valid(metadata_json) THEN metadata_json ELSE '{}' END,
              '$.transcript_status', 'complete',
              '$.capture_started_at_ms', ?
            ), updated_at = ? WHERE id = ? AND session_id = ? AND deleted_at IS NULL`,
            params: [
              captureStartedAtMs,
              now,
              `session-audio:${sessionId}`,
              sessionId,
            ],
          },
        ]);
      }
    }
    return sessionId;
  } catch (error) {
    if (importedAudioFile?.exists) importedAudioFile.delete();
    await deleteSession(sessionId).catch(() => {});
    throw error;
  } finally {
    if (archive.audio?.file.exists) archive.audio.file.delete();
  }
}

function parseMeetingPackage(raw: string): MeetingPackage {
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    throw new Error("This file is not a valid Anarlog meeting package.");
  }
  if (!value || typeof value !== "object") throw invalidPackage();
  const packageValue = value as Partial<MeetingPackage>;
  const meeting = packageValue.meeting;
  if (
    packageValue.format !== FORMAT ||
    packageValue.version !== VERSION ||
    typeof packageValue.exported_at !== "string" ||
    !Number.isFinite(Date.parse(packageValue.exported_at)) ||
    !meeting ||
    typeof meeting.title !== "string" ||
    meeting.title.length > 500 ||
    typeof meeting.created_at !== "string" ||
    !Number.isFinite(Date.parse(meeting.created_at)) ||
    typeof meeting.note !== "string" ||
    (meeting.speaker_context !== null &&
      (typeof meeting.speaker_context !== "object" ||
        Array.isArray(meeting.speaker_context))) ||
    !Array.isArray(meeting.summaries) ||
    !Array.isArray(packageValue.transcripts) ||
    (packageValue.audio !== null &&
      (!packageValue.audio || typeof packageValue.audio !== "object"))
  )
    throw invalidPackage();
  for (const summary of meeting.summaries) {
    if (
      !summary ||
      typeof summary.title !== "string" ||
      typeof summary.body !== "string" ||
      !["markdown", "prosemirror_json"].includes(summary.body_format) ||
      typeof summary.generation_metadata_json !== "string"
    )
      throw invalidPackage();
    try {
      const metadata: unknown = JSON.parse(summary.generation_metadata_json);
      if (!metadata || typeof metadata !== "object" || Array.isArray(metadata))
        throw invalidPackage();
    } catch {
      throw invalidPackage();
    }
  }
  const transcriptIds = new Set<string>();
  for (const transcript of packageValue.transcripts) {
    if (
      !transcript ||
      typeof transcript.id !== "string" ||
      transcript.id.length === 0 ||
      transcript.id.length > 128 ||
      transcriptIds.has(transcript.id) ||
      typeof transcript.source !== "string" ||
      typeof transcript.provider !== "string" ||
      typeof transcript.model !== "string" ||
      typeof transcript.language !== "string" ||
      typeof transcript.memo !== "string" ||
      typeof transcript.words_json !== "string" ||
      typeof transcript.speaker_hints_json !== "string" ||
      (transcript.metadata_json !== undefined &&
        typeof transcript.metadata_json !== "string") ||
      !Number.isSafeInteger(transcript.started_at_ms) ||
      (transcript.ended_at_ms !== null &&
        !Number.isSafeInteger(transcript.ended_at_ms))
    )
      throw invalidPackage();
    transcriptIds.add(transcript.id);
    try {
      if (
        !Array.isArray(JSON.parse(transcript.words_json)) ||
        !Array.isArray(JSON.parse(transcript.speaker_hints_json)) ||
        (transcript.metadata_json !== undefined &&
          (!JSON.parse(transcript.metadata_json) ||
            typeof JSON.parse(transcript.metadata_json) !== "object" ||
            Array.isArray(JSON.parse(transcript.metadata_json))))
      )
        throw invalidPackage();
    } catch {
      throw invalidPackage();
    }
  }
  if (packageValue.audio) {
    if (
      typeof packageValue.audio.content_type !== "string" ||
      packageValue.audio.content_type.length > 512 ||
      packageValue.audio.content_type.trim() !==
        packageValue.audio.content_type ||
      /[\u0000-\u001f\u007f]/u.test(packageValue.audio.content_type) ||
      typeof packageValue.audio.filename !== "string" ||
      typeof packageValue.audio.sha256 !== "string" ||
      !/^[0-9a-f]{64}$/u.test(packageValue.audio.sha256) ||
      !Number.isSafeInteger(packageValue.audio.size_bytes) ||
      packageValue.audio.size_bytes <= 0 ||
      packageValue.audio.size_bytes > MAX_TRANSCRIPTION_AUDIO_BYTES
    )
      throw invalidPackage();
    safeAudioFilename(packageValue.audio.filename);
  }
  return packageValue as MeetingPackage;
}

function parseObject(value: string): Record<string, unknown> {
  try {
    const parsed: unknown = JSON.parse(value);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

function noteText(body: string, format: string): string {
  if (format === "markdown") return stripMarkdownTitle(body).text;
  return docToPlainText(body).text;
}

function safeName(value: string): string {
  return (
    value
      .trim()
      .replace(/[\\/:*?"<>|\u0000-\u001f]/gu, "_")
      .slice(0, 80) || "meeting"
  );
}

function safeAudioFilename(value: string): string {
  if (
    value.length === 0 ||
    value.length > 160 ||
    value === "." ||
    value === ".." ||
    /[\\/\u0000-\u001f\u007f]/u.test(value)
  ) {
    throw invalidPackage();
  }
  return value;
}

function safeAudioContentType(value: string): string {
  if (
    value.length === 0 ||
    value.length > 512 ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    throw invalidPackage();
  }
  return value;
}

function invalidPackage(): Error {
  return new Error("This file is not a supported Anarlog meeting package.");
}
