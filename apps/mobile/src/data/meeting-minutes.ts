import {
  MeetingNotesProviderResponseSchema,
  MeetingMinutesRequestSchema,
  validateEvidenceReferences,
  type MeetingMinutesRequest,
} from "@anlg/meeting-schema";

export function cloudflareMeetingMinutesRequest(
  baseUrl: string,
  accessToken: string,
  payload: MeetingMinutesRequest,
) {
  const request = MeetingMinutesRequestSchema.parse(payload);
  return {
    url: `${baseUrl.replace(/\/+$/u, "")}/v1/meeting-minutes`,
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${accessToken}`,
    },
    body: JSON.stringify(request),
  };
}

export function readCloudflareMeetingMinutes(
  payload: unknown,
  segments: MeetingMinutesRequest["segments"],
) {
  const response = MeetingNotesProviderResponseSchema.parse(payload);
  validateEvidenceReferences(response.minutes, segments);
  return response;
}

export function renderMeetingMinutes(
  response: ReturnType<typeof readCloudflareMeetingMinutes>,
  language: string,
): string {
  const vietnamese = language.toLowerCase().startsWith("vi");
  const labels = vietnamese
    ? {
        summary: "Tóm tắt",
        decisions: "Quyết định",
        actions: "Việc cần làm",
        questions: "Vấn đề còn mở",
        suggestions: "Gợi ý của AI",
        owner: "Người phụ trách",
        deadline: "Thời hạn",
        status: "Trạng thái",
        evidence: "Dẫn chứng",
        confirmed: "Đã xác nhận",
        needsConfirmation: "Cần xác nhận",
        suggested: "Đề xuất",
      }
    : {
        summary: "Summary",
        decisions: "Decisions",
        actions: "Action items",
        questions: "Open questions",
        suggestions: "AI suggestions",
        owner: "Owner",
        deadline: "Deadline",
        status: "Status",
        evidence: "Evidence",
        confirmed: "Confirmed",
        needsConfirmation: "Needs confirmation",
        suggested: "Suggested",
      };
  const { minutes } = response;
  const inlineCode = (source: string) => {
    const runs = source.match(/`+/gu) ?? [];
    const fence = "`".repeat(Math.max(1, ...runs.map((run) => run.length + 1)));
    return `${fence}${source.replace(/\s+/gu, " ")}${fence}`;
  };
  const evidence = (quotes: { segment_id: string; quote: string }[]) =>
    `  ${labels.evidence}: ${quotes
      .map(
        ({ segment_id, quote }) =>
          `${inlineCode(quote)} [${segment_id}](anarlog-evidence://${encodeURIComponent(segment_id)})`,
      )
      .join(", ")}`;
  const sections = [`## ${labels.summary}\n\n${minutes.summary}`];
  if (minutes.decisions.length)
    sections.push(
      `## ${labels.decisions}\n\n${minutes.decisions.map((item) => `- ${item.text}\n${evidence(item.evidence_quotes)}`).join("\n")}`,
    );
  if (minutes.action_items.length)
    sections.push(
      `## ${labels.actions}\n\n${minutes.action_items
        .map((item) => {
          const status =
            item.status === "confirmed"
              ? labels.confirmed
              : item.status === "suggested"
                ? labels.suggested
                : labels.needsConfirmation;
          const details = [
            item.owner_text ? `${labels.owner}: ${item.owner_text}` : null,
            item.deadline_text
              ? `${labels.deadline}: ${item.deadline_text}`
              : null,
            `${labels.status}: ${status}`,
          ].filter(Boolean);
          return `- ${item.task}${details.length ? `\n  ${details.join(" · ")}` : ""}\n${evidence(item.evidence_quotes)}`;
        })
        .join("\n")}`,
    );
  if (minutes.open_questions.length)
    sections.push(
      `## ${labels.questions}\n\n${minutes.open_questions.map((item) => `- ${item.text}\n${evidence(item.evidence_quotes)}`).join("\n")}`,
    );
  if (minutes.suggestions.length)
    sections.push(
      `## ${labels.suggestions}\n\n${minutes.suggestions.map(({ text }) => `- ${text}`).join("\n")}`,
    );
  return sections.join("\n\n");
}
