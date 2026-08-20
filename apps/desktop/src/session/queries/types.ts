export type SessionRecord = {
  id: string;
  user_id: string;
  created_at: string;
  folder_id: string;
  event_json: string;
  title: string;
  raw_md: string;
  raw_template_id: string;
  locked: boolean;
};

export type SessionChanges = Partial<
  Pick<
    SessionRecord,
    | "created_at"
    | "event_json"
    | "folder_id"
    | "locked"
    | "raw_md"
    | "raw_template_id"
    | "title"
  >
>;

export type SessionSummaryRecord = {
  id: string;
  title: string;
  created_at: string;
};

export type EnhancedNoteRecord = {
  id: string;
  sessionId: string;
  title: string;
  content: string;
  templateId: string;
  position: number;
};

export type SessionParticipantRecord = {
  id: string;
  sessionId: string;
  humanId: string;
  source: string;
  name: string;
  email: string;
  jobTitle: string;
  linkedinUsername: string;
  organizationId: string;
  organizationName: string;
};
