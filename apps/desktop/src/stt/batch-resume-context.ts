export type BatchResumeContext = {
  promotion: "whole_session";
};

export function serializeBatchResumeContext(context: BatchResumeContext) {
  return JSON.stringify(context);
}

export function parseBatchResumeContext(
  value: string | null | undefined,
): BatchResumeContext | null {
  if (!value) return null;
  try {
    const parsed: unknown = JSON.parse(value);
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      !("promotion" in parsed) ||
      parsed.promotion !== "whole_session"
    ) {
      return null;
    }
    return { promotion: "whole_session" };
  } catch {
    return null;
  }
}
