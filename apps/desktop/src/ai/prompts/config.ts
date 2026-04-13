export type TaskType = "enhance" | "title";

export const AVAILABLE_FILTERS = ["transcript", "url"] as const;

export const TASK_CONFIGS = [
  {
    type: "enhance" as const,
    label: "Enhance Notes",
    description: "Generates structured meeting summaries from transcripts",
    variables: [
      "content",
      "session",
      "participants",
      "template",
      "pre_meeting_memo",
      "post_meeting_memo",
    ],
  },
  {
    type: "title" as const,
    label: "Title Generation",
    description: "Generates a title for the meeting note",
    variables: ["enhanced_note"],
  },
] as const;
