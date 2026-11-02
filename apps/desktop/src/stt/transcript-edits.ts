type TranscriptEditSource = {
  transcriptId: string;
  metadataJson?: string;
};

type EditableSegment = {
  id: string;
  text: string;
};

export function applyStoredTranscriptEdits<T extends EditableSegment>(
  segments: T[],
  transcripts: TranscriptEditSource[],
): T[] {
  const edits = new Map<string, string>();
  for (const { transcriptId, metadataJson } of transcripts) {
    try {
      const metadata: unknown = JSON.parse(metadataJson || "{}");
      if (!metadata || typeof metadata !== "object" || Array.isArray(metadata))
        continue;
      const candidate = (metadata as { transcript_edits?: unknown })
        .transcript_edits;
      if (
        !candidate ||
        typeof candidate !== "object" ||
        Array.isArray(candidate)
      )
        continue;
      for (const [segmentId, edit] of Object.entries(candidate)) {
        if (
          segmentId.startsWith(`${transcriptId}:`) &&
          edit &&
          typeof edit === "object" &&
          "text" in edit &&
          typeof edit.text === "string"
        ) {
          edits.set(segmentId.slice(transcriptId.length + 1), edit.text);
        }
      }
    } catch {
      // Invalid metadata must not prevent the recognized transcript from rendering.
    }
  }
  if (edits.size === 0) return segments;
  return segments.map((segment) => {
    const text = edits.get(segment.id);
    return text === undefined ? segment : { ...segment, text };
  });
}
