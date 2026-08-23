export type AutomationRunRecord = {
  at: string;
  status: "success" | "error";
  detail: string;
};

export function parseAutomationRunRecord(
  value: string | undefined,
): AutomationRunRecord | null {
  if (!value) {
    return null;
  }
  try {
    const parsed = JSON.parse(value);
    if (
      typeof parsed?.at === "string" &&
      (parsed.status === "success" || parsed.status === "error") &&
      typeof parsed.detail === "string"
    ) {
      return parsed as AutomationRunRecord;
    }
  } catch {
    // fall through
  }
  return null;
}

export type AutomationTargetRef = {
  id: string;
  name: string;
};

export function parseAutomationTargetRef(
  value: string | undefined,
): AutomationTargetRef | null {
  if (!value) {
    return null;
  }
  try {
    const parsed = JSON.parse(value);
    if (typeof parsed?.id === "string" && typeof parsed.name === "string") {
      return { id: parsed.id, name: parsed.name };
    }
  } catch {
    // fall through
  }
  return null;
}
