import { commands as localApiCommands } from "@anlg/plugin-local-api";

import { getStoredSettingValues, setSettingValue } from "~/settings/queries";

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

export async function runMeetingCompletedAutomations(
  sessionId: string,
): Promise<void> {
  try {
    await runMarkdownExport(sessionId);
  } catch (error) {
    console.error("[automations] meeting.completed run failed", error);
  }
}

async function runMarkdownExport(sessionId: string): Promise<void> {
  const { values } = await getStoredSettingValues();
  if (!values.automation_markdown_export_enabled) {
    return;
  }
  const directory = (values.automation_markdown_export_directory ?? "").trim();
  if (!directory) {
    return;
  }

  const record: AutomationRunRecord = {
    at: new Date().toISOString(),
    status: "success",
    detail: "",
  };
  try {
    const result = await localApiCommands.exportMeetingMarkdown(
      sessionId,
      directory,
    );
    if (result.status === "error") {
      throw new Error(result.error);
    }
    record.detail = result.data;
  } catch (error) {
    record.status = "error";
    record.detail = error instanceof Error ? error.message : String(error);
    console.error("[automations] markdown export failed", error);
  }
  await setSettingValue(
    "automation_markdown_export_last_run",
    JSON.stringify(record),
  );
}
