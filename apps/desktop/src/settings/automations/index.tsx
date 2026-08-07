import { Icon } from "@iconify-icon/react";
import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowRight,
  Eye,
  FloppyDisk,
  Lightning,
  Play,
  Sparkle,
} from "@phosphor-icons/react";
import { useMutation } from "@tanstack/react-query";
import { useState } from "react";

import { Badge } from "@anlg/ui/components/ui/badge";
import { Button } from "@anlg/ui/components/ui/button";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

import {
  AutomationLastRunLine,
  LinearIssuesConfig,
  MarkdownExportConfig,
  NotionUpdateConfig,
  SlackRecapConfig,
} from "./starter-config";

import { useBillingAccess } from "~/auth/billing-context";
import { parseAutomationTargetRef } from "~/automations/engine";
import { SettingsHydrationBoundary } from "~/settings/hydration-boundary";
import { SettingsPageTitle } from "~/settings/page-title";
import {
  setSettingValue,
  setSettingValues,
  useStoredSettingValue,
  useStoredSettingValues,
} from "~/settings/queries";
import type { SettingValues } from "~/settings/schema";
import { StandardContentWrapper } from "~/shared/main";

const STARTER_AUTOMATIONS = {
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

type StarterId = keyof typeof STARTER_AUTOMATIONS;

export function TabContentAutomations() {
  return (
    <StandardContentWrapper>
      <SettingsHydrationBoundary>
        <div className="bg-card dark:bg-accent flex w-full flex-1 flex-col overflow-hidden">
          <div className="scroll-fade-y scrollbar-hide h-full w-full flex-1 overflow-y-auto p-6">
            <SettingsAutomations />
          </div>
        </div>
      </SettingsHydrationBoundary>
    </StandardContentWrapper>
  );
}

export function SettingsAutomations() {
  const { t } = useLingui();
  const billing = useBillingAccess();
  const storedDraft = useStoredSettingValue("automation_draft_template");
  const starters = [
    {
      id: "slack-recap",
      title: t`Share a meeting recap in Slack`,
      description: t`Post a meeting recap to a Slack channel.`,
      icon: (
        <Icon
          icon="logos:slack-icon"
          width={22}
          height={22}
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
      icon: (
        <Icon
          icon="logos:notion-icon"
          width={22}
          height={22}
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
      icon: (
        <Icon
          icon="logos:linear-icon"
          width={22}
          height={22}
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
      icon: (
        <img
          src="/assets/markdown-mark.svg"
          alt=""
          className="h-5 w-auto dark:invert"
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
  ] as const;
  const initialDraftId =
    storedDraft.value &&
    starters.some((starter) => starter.id === storedDraft.value)
      ? storedDraft.value
      : "";
  const [draftTemplateId, setDraftTemplateId] = useState(initialDraftId);
  const [showPreview, setShowPreview] = useState(false);
  const draft = starters.find((starter) => starter.id === draftTemplateId);
  const saveDraftMutation = useMutation({
    mutationKey: ["automation-draft-template"],
    mutationFn: () =>
      setSettingValue("automation_draft_template", draftTemplateId),
    onSuccess: () => sonnerToast.success(t`Automation draft saved`),
    onError: () => sonnerToast.error(t`Could not save the automation draft`),
  });

  const handleSaveDraft = () => {
    if (!billing.isPro) {
      billing.upgradeToPro();
      return;
    }
    saveDraftMutation.mutate();
  };

  const { values: settingValues } = useStoredSettingValues();
  const isStarterEnabled = (id: StarterId) =>
    Boolean(settingValues[STARTER_AUTOMATIONS[id].enabledKey]);
  const isStarterReady = (id: StarterId) => {
    const raw = settingValues[STARTER_AUTOMATIONS[id].targetKey] ?? "";
    return id === "markdown-export"
      ? raw.trim().length > 0
      : parseAutomationTargetRef(raw) !== null;
  };
  const starterReadinessHint = (id: StarterId) => {
    switch (id) {
      case "markdown-export":
        return t`Choose an export folder first.`;
      case "slack-recap":
        return t`Choose a Slack channel first.`;
      case "linear-action-items":
        return t`Choose a Linear team first.`;
      case "notion-project-notes":
        return t`Choose a Notion page first.`;
    }
  };

  const setStarterEnabledMutation = useMutation({
    mutationKey: ["automation-starter-enabled"],
    mutationFn: ({ id, enabled }: { id: StarterId; enabled: boolean }) => {
      const updates: SettingValues = { automation_draft_template: id };
      updates[STARTER_AUTOMATIONS[id].enabledKey] = enabled;
      return setSettingValues(updates);
    },
    onSuccess: (_, { enabled }) =>
      sonnerToast.success(
        enabled ? t`Automation enabled` : t`Automation disabled`,
      ),
    onError: () => sonnerToast.error(t`Could not update the automation`),
  });

  const handleEnableStarter = (id: StarterId) => {
    if (!billing.isPro) {
      billing.upgradeToPro();
      return;
    }
    setStarterEnabledMutation.mutate({ id, enabled: true });
  };

  return (
    <div className="mx-auto flex w-full max-w-5xl flex-col gap-6 pb-8">
      <div className="flex flex-col gap-2">
        <SettingsPageTitle title={<Trans>Automations</Trans>} />
        <p className="text-muted-foreground max-w-2xl text-sm leading-relaxed">
          <Trans>
            Automate what happens before, during, or after meetings based on the
            conditions you choose.
          </Trans>
        </p>
      </div>

      <section className="flex flex-col gap-3" aria-labelledby="starters-title">
        <h3 id="starters-title" className="text-sm font-semibold">
          <Trans>Get started</Trans>
        </h3>
        <div className="flex flex-col gap-1">
          {starters.map((starter) => {
            const selected = starter.id === draftTemplateId;
            return (
              <button
                key={starter.id}
                type="button"
                aria-pressed={selected}
                onClick={() => {
                  setDraftTemplateId(starter.id);
                  setShowPreview(false);
                }}
                className={cn([
                  "group flex w-full items-center gap-3 px-1 py-2.5 text-left",
                  "focus-visible:ring-ring rounded-lg outline-none focus-visible:ring-2",
                ])}
              >
                <span className="flex size-8 shrink-0 items-center justify-center">
                  {starter.icon}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-sm font-medium">
                    {starter.title}
                  </span>
                  <span className="text-muted-foreground mt-0.5 block text-xs leading-relaxed">
                    {starter.description}
                  </span>
                </span>
                {isStarterEnabled(starter.id) ? (
                  <Badge variant="outline" size="sm">
                    <Trans>Enabled</Trans>
                  </Badge>
                ) : null}
                <ArrowRight
                  className={cn([
                    "size-5 shrink-0 transition-transform group-hover:translate-x-0.5",
                    selected ? "text-primary" : "text-muted-foreground",
                  ])}
                  aria-hidden="true"
                />
              </button>
            );
          })}
        </div>
      </section>

      {draft ? (
        <section
          className="border-border bg-background overflow-hidden rounded-2xl border"
          aria-labelledby="automation-draft-title"
        >
          <div className="border-border flex flex-wrap items-start justify-between gap-3 border-b px-5 py-4">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <Lightning className="text-primary" size={17} weight="fill" />
                <h3
                  id="automation-draft-title"
                  className="truncate text-sm font-semibold"
                >
                  {draft.title}
                </h3>
                {isStarterEnabled(draft.id) ? (
                  <Badge variant="outline">
                    <Trans>Enabled</Trans>
                  </Badge>
                ) : (
                  <Badge variant="outline">
                    <Trans>Draft</Trans>
                  </Badge>
                )}
              </div>
              <p className="text-muted-foreground mt-1 text-xs">
                <Trans>Steps run from top to bottom.</Trans>
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => setShowPreview((visible) => !visible)}
              >
                <Eye size={14} />
                {showPreview ? (
                  <Trans>Hide preview</Trans>
                ) : (
                  <Trans>Preview</Trans>
                )}
              </Button>
              <Button
                type="button"
                size="sm"
                variant="outline"
                disabled
                title={t`Test runs are not available yet.`}
              >
                <Play size={14} />
                <Trans>Test</Trans>
              </Button>
              <Button
                type="button"
                size="sm"
                onClick={handleSaveDraft}
                disabled={!billing.isReady || saveDraftMutation.isPending}
              >
                <FloppyDisk size={14} />
                {billing.isPro ? (
                  <Trans>Save draft</Trans>
                ) : (
                  <Trans>Upgrade to save</Trans>
                )}
              </Button>
              {isStarterEnabled(draft.id) ? (
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    setStarterEnabledMutation.mutate({
                      id: draft.id,
                      enabled: false,
                    })
                  }
                  disabled={
                    !billing.isReady || setStarterEnabledMutation.isPending
                  }
                >
                  <Trans>Disable</Trans>
                </Button>
              ) : (
                <Button
                  type="button"
                  size="sm"
                  onClick={() => handleEnableStarter(draft.id)}
                  disabled={
                    !billing.isReady ||
                    setStarterEnabledMutation.isPending ||
                    (billing.isPro && !isStarterReady(draft.id))
                  }
                  title={
                    billing.isPro && !isStarterReady(draft.id)
                      ? starterReadinessHint(draft.id)
                      : undefined
                  }
                >
                  <Lightning size={14} weight="fill" />
                  {billing.isPro ? (
                    <Trans>Save &amp; enable</Trans>
                  ) : (
                    <Trans>Upgrade to enable</Trans>
                  )}
                </Button>
              )}
            </div>
          </div>

          <div className="flex flex-col gap-2 p-5">
            {draft.steps.map((step, index) => (
              <div key={`${step.kind}-${step.title}`}>
                <div className="border-border bg-card flex items-start gap-3 rounded-xl border p-4">
                  <span
                    className={cn([
                      "mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-full text-xs font-semibold",
                      step.kind === "ai"
                        ? "bg-violet-100 text-violet-700 dark:bg-violet-950 dark:text-violet-300"
                        : step.kind === "trigger"
                          ? "bg-amber-100 text-amber-700 dark:bg-amber-950 dark:text-amber-300"
                          : "bg-blue-100 text-blue-700 dark:bg-blue-950 dark:text-blue-300",
                    ])}
                  >
                    {step.kind === "ai" ? <Sparkle size={13} /> : index + 1}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="flex flex-wrap items-center gap-2">
                      <span className="text-sm font-medium">{step.title}</span>
                      <Badge variant="outline" size="sm">
                        {step.kind === "ai" ? (
                          <Trans>AI step</Trans>
                        ) : step.kind === "trigger" ? (
                          <Trans>Trigger</Trans>
                        ) : (
                          <Trans>Action</Trans>
                        )}
                      </Badge>
                    </span>
                    <span className="text-muted-foreground mt-1 block text-xs leading-relaxed">
                      {step.detail}
                    </span>
                  </span>
                </div>
                {index < draft.steps.length - 1 ? (
                  <div className="text-muted-foreground flex h-6 items-center pl-6">
                    <ArrowRight className="rotate-90" size={13} />
                  </div>
                ) : null}
              </div>
            ))}
          </div>

          <div className="border-border border-t px-5 py-4">
            {draft.id === "markdown-export" ? (
              <MarkdownExportConfig />
            ) : draft.id === "slack-recap" ? (
              <SlackRecapConfig />
            ) : draft.id === "linear-action-items" ? (
              <LinearIssuesConfig />
            ) : (
              <NotionUpdateConfig />
            )}
            <AutomationLastRunLine
              settingKey={STARTER_AUTOMATIONS[draft.id].lastRunKey}
            />
          </div>

          {showPreview ? (
            <div className="border-border bg-muted/35 border-t px-5 py-4">
              <div className="flex items-start gap-3">
                <Eye className="text-muted-foreground mt-0.5" size={15} />
                <div>
                  <h4 className="text-xs font-semibold">
                    <Trans>Expected output</Trans>
                  </h4>
                  <p className="text-muted-foreground mt-1 text-xs leading-relaxed">
                    {draft.preview}
                  </p>
                </div>
              </div>
            </div>
          ) : null}
        </section>
      ) : (
        <section className="border-border bg-muted/20 flex min-h-56 flex-col items-center justify-center rounded-2xl border border-dashed px-6 py-10 text-center">
          <span className="bg-background border-border flex size-11 items-center justify-center rounded-2xl border">
            <Lightning className="text-muted-foreground" size={20} />
          </span>
          <h3 className="mt-4 text-sm font-semibold">
            <Trans>No automation draft yet</Trans>
          </h3>
          <p className="text-muted-foreground mt-1 max-w-sm text-xs leading-relaxed">
            <Trans>Choose a starter or describe an automation in Chat.</Trans>
          </p>
        </section>
      )}
    </div>
  );
}
