import { Icon } from "@iconify-icon/react";
import { useLingui } from "@lingui/react/macro";
import type { ReactNode } from "react";

export const STARTER_AUTOMATIONS = {
  "slack-recap": {
    enabledKey: "automation_slack_recap_enabled",
    targetKey: "automation_slack_recap_channel",
    lastRunKey: "automation_slack_recap_last_run",
  },
  "notion-project-notes": {
    enabledKey: "automation_notion_update_enabled",
    targetKey: "automation_notion_update_page",
    lastRunKey: "automation_notion_update_last_run",
  },
  "linear-action-items": {
    enabledKey: "automation_linear_issues_enabled",
    targetKey: "automation_linear_issues_team",
    lastRunKey: "automation_linear_issues_last_run",
  },
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
      id: "slack-recap",
      title: t`Share a meeting recap in Slack`,
      description: t`Post a meeting recap to a Slack channel.`,
      renderIcon: (size) => (
        <Icon
          icon="logos:slack-icon"
          width={size}
          height={size}
          aria-hidden="true"
        />
      ),
      steps: [
        {
          kind: "trigger",
          title: t`Meeting ends`,
          detail: t`Runs once the AI summary for the meeting is ready.`,
        },
        {
          kind: "ai",
          title: t`Use the AI meeting summary`,
          detail: t`Take the enhanced note with decisions and action items.`,
        },
        {
          kind: "action",
          title: t`Post to a channel`,
          detail: t`Send the recap to the selected Slack channel.`,
        },
      ],
      preview: t`A Slack message with the meeting title and recap.`,
    },
    {
      id: "notion-project-notes",
      title: t`Update project notes in Notion`,
      description: t`Add meeting decisions and follow-ups to a Notion project.`,
      renderIcon: (size) => (
        <Icon
          icon="logos:notion-icon"
          width={size}
          height={size}
          aria-hidden="true"
        />
      ),
      steps: [
        {
          kind: "trigger",
          title: t`Meeting ends`,
          detail: t`Runs once the AI summary for the meeting is ready.`,
        },
        {
          kind: "ai",
          title: t`Use the AI meeting summary`,
          detail: t`Take the enhanced note with decisions and follow-ups.`,
        },
        {
          kind: "action",
          title: t`Append the meeting update`,
          detail: t`Add a dated update to the selected Notion page.`,
        },
      ],
      preview: t`A dated Notion update with the meeting summary.`,
    },
    {
      id: "linear-action-items",
      title: t`Turn action items into Linear issues`,
      description: t`Turn assigned follow-ups into Linear issue drafts.`,
      renderIcon: (size) => (
        <Icon
          icon="logos:linear-icon"
          width={size}
          height={size}
          aria-hidden="true"
        />
      ),
      steps: [
        {
          kind: "trigger",
          title: t`Meeting ends`,
          detail: t`Runs once the AI summary for the meeting is ready.`,
        },
        {
          kind: "ai",
          title: t`Collect action items`,
          detail: t`Use the meeting's action items and summary tasks.`,
        },
        {
          kind: "action",
          title: t`Create Linear issues`,
          detail: t`File each action item as an issue in the selected team.`,
        },
      ],
      preview: t`Linear issues created from meeting follow-ups.`,
    },
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
