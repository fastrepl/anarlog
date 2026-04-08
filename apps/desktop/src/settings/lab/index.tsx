import { AgentIntegrations } from "./agent-integrations";
import { DownloadButtons } from "./download-buttons";
import { LocalLlmTester } from "./local-llm-tester";
import { V1p1PreviewToggle } from "./v1p1-preview-toggle";

import { SettingsPageTitle } from "~/settings/page-title";

export function SettingsLab() {
  return (
    <div className="flex flex-col gap-6">
      <SettingsPageTitle title="Lab" />
      <DownloadButtons />
      <V1p1PreviewToggle />
      <AgentIntegrations />
      <LocalLlmTester />
    </div>
  );
}
