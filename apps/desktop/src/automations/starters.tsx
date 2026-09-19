import { useLingui } from "@lingui/react/macro";
import type { ReactNode } from "react";

export const STARTER_AUTOMATIONS = {
  "markdown-export": {
    enabledKey: "automation_markdown_export_enabled",
    targetKey: "automation_markdown_export_directory",
    lastRunKey: "automation_markdown_export_last_run",
  },
} as const;

export type StarterId = keyof typeof STARTER_AUTOMATIONS;

export function isStarterId(
  value: string | null | undefined,
): value is StarterId {
  return typeof value === "string" && value in STARTER_AUTOMATIONS;
}

export type StarterAutomation = {
  id: StarterId;
  title: string;
  description: string;
  renderIcon: (size: number) => ReactNode;
  steps: ReadonlyArray<{
    kind: "trigger" | "ai" | "action";
    title: string;
    detail: string;
  }>;
  preview: string;
};

export function useStarterAutomations(): StarterAutomation[] {
  const { t } = useLingui();

  return [
    {
      id: "markdown-export",
      title: t`Export every meeting as Markdown`,
      description: t`Save completed meetings as local Markdown files.`,
      renderIcon: (size) => (
        <img
          src="/assets/markdown-mark.svg"
          alt=""
          style={{ height: size }}
          className="w-auto dark:invert"
        />
      ),
      steps: [
        {
          kind: "trigger",
          title: t`Meeting ends`,
          detail: t`Wait until the transcript and note are complete.`,
        },
        {
          kind: "action",
          title: t`Render canonical Markdown`,
          detail: t`Combine metadata, summary, notes, and transcript.`,
        },
        {
          kind: "action",
          title: t`Write to a folder`,
          detail: t`Use a stable filename in the configured export directory.`,
        },
      ],
      preview: t`A Markdown file with the note, summary, and transcript.`,
    },
  ];
}
