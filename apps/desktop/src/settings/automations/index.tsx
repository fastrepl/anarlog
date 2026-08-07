import { Icon } from "@iconify-icon/react";
import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowRight,
  Eye,
  FloppyDisk,
  FolderOpen,
  Lightning,
  Play,
  Sparkle,
} from "@phosphor-icons/react";
import { useMutation } from "@tanstack/react-query";
import { open as selectFolder } from "@tauri-apps/plugin-dialog";
import { useState } from "react";

import { Badge } from "@anlg/ui/components/ui/badge";
import { Button } from "@anlg/ui/components/ui/button";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn, formatDistanceToNow } from "@anlg/utils";

import { useBillingAccess } from "~/auth/billing-context";
import { parseAutomationRunRecord } from "~/automations/engine";
import { SettingsHydrationBoundary } from "~/settings/hydration-boundary";
import { SettingsPageTitle } from "~/settings/page-title";
import {
  setSettingValue,
  setSettingValues,
  useStoredSettingValue,
} from "~/settings/queries";
import { StandardContentWrapper } from "~/shared/main";

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
          detail: t`Use the completed meeting, transcript, and notes.`,
        },
        {
          kind: "ai",
          title: t`Generate a concise recap`,
          detail: t`Summarize decisions, action items, and open questions.`,
        },
        {
          kind: "action",
          title: t`Create a Slack canvas`,
          detail: t`Write the recap with a link to the Anarlog note.`,
        },
        {
          kind: "action",
          title: t`Post to a channel`,
          detail: t`Send the canvas link to the selected Slack channel.`,
        },
      ],
      preview: t`A Slack canvas with the recap and source note.`,
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
          detail: t`Use the completed meeting, transcript, and notes.`,
        },
        {
          kind: "ai",
          title: t`Extract project updates`,
          detail: t`Identify decisions, owners, deadlines, and unresolved questions.`,
        },
        {
          kind: "action",
          title: t`Find the project page`,
          detail: t`Match the meeting to the configured Notion project.`,
        },
        {
          kind: "action",
          title: t`Append the meeting update`,
          detail: t`Add a dated update linked to the Anarlog note.`,
        },
      ],
      preview: t`A dated Notion update with decisions and owners.`,
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
          detail: t`Use the completed meeting, transcript, and notes.`,
        },
        {
          kind: "ai",
          title: t`Extract assigned action items`,
          detail: t`Keep concrete work with an owner or next step.`,
        },
        {
          kind: "action",
          title: t`Prepare Linear issues`,
          detail: t`Map titles, descriptions, owners, and the source-note link.`,
        },
      ],
      preview: t`Linear issue drafts from assigned meeting follow-ups.`,
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

  const markdownExportEnabled =
    useStoredSettingValue("automation_markdown_export_enabled").value ?? false;
  const markdownExportDirectory = (
    useStoredSettingValue("automation_markdown_export_directory").value ?? ""
  ).trim();
  const markdownExportLastRun = parseAutomationRunRecord(
    useStoredSettingValue("automation_markdown_export_last_run").value,
  );
  const isMarkdownDraft = draft?.id === "markdown-export";

  const chooseFolderMutation = useMutation({
    mutationKey: ["automation-markdown-export-folder"],
    mutationFn: async () => {
      const selected = await selectFolder({
        title: t`Choose export folder`,
        directory: true,
        multiple: false,
        defaultPath: markdownExportDirectory || undefined,
      });
      if (typeof selected === "string" && selected) {
        await setSettingValue("automation_markdown_export_directory", selected);
      }
    },
    onError: () => sonnerToast.error(t`Could not update the export folder`),
  });
  const setMarkdownExportEnabledMutation = useMutation({
    mutationKey: ["automation-markdown-export-enabled"],
    mutationFn: (enabled: boolean) =>
      setSettingValues({
        automation_markdown_export_enabled: enabled,
        automation_draft_template: draftTemplateId,
      }),
    onSuccess: (_, enabled) =>
      sonnerToast.success(
        enabled ? t`Automation enabled` : t`Automation disabled`,
      ),
    onError: () => sonnerToast.error(t`Could not update the automation`),
  });

  const handleEnableMarkdownExport = () => {
    if (!billing.isPro) {
      billing.upgradeToPro();
      return;
    }
    setMarkdownExportEnabledMutation.mutate(true);
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
                {starter.id === "markdown-export" && markdownExportEnabled ? (
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
                {isMarkdownDraft && markdownExportEnabled ? (
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
                title={t`Test runs will be available when execution is connected.`}
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
              {isMarkdownDraft ? (
                markdownExportEnabled ? (
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    onClick={() =>
                      setMarkdownExportEnabledMutation.mutate(false)
                    }
                    disabled={
                      !billing.isReady ||
                      setMarkdownExportEnabledMutation.isPending
                    }
                  >
                    <Trans>Disable</Trans>
                  </Button>
                ) : (
                  <Button
                    type="button"
                    size="sm"
                    onClick={handleEnableMarkdownExport}
                    disabled={
                      !billing.isReady ||
                      setMarkdownExportEnabledMutation.isPending ||
                      (billing.isPro && !markdownExportDirectory)
                    }
                    title={
                      billing.isPro && !markdownExportDirectory
                        ? t`Choose an export folder first.`
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
                )
              ) : (
                <Button
                  type="button"
                  size="sm"
                  disabled
                  title={t`Enabling will be available when execution is connected.`}
                >
                  <Lightning size={14} weight="fill" />
                  <Trans>Save &amp; enable</Trans>
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

          {isMarkdownDraft ? (
            <div className="border-border border-t px-5 py-4">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div className="min-w-0">
                  <h4 className="text-xs font-semibold">
                    <Trans>Export folder</Trans>
                  </h4>
                  <p className="text-muted-foreground mt-1 truncate text-xs">
                    {markdownExportDirectory || (
                      <Trans>No folder selected yet.</Trans>
                    )}
                  </p>
                </div>
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={() => chooseFolderMutation.mutate()}
                  disabled={chooseFolderMutation.isPending}
                >
                  <FolderOpen size={14} />
                  <Trans>Choose folder</Trans>
                </Button>
              </div>
              {markdownExportLastRun ? (
                <p
                  className={cn([
                    "mt-3 truncate text-xs",
                    markdownExportLastRun.status === "error"
                      ? "text-destructive"
                      : "text-muted-foreground",
                  ])}
                  title={markdownExportLastRun.detail}
                >
                  {markdownExportLastRun.status === "success" ? (
                    <Trans>
                      Last exported{" "}
                      {formatDistanceToNow(new Date(markdownExportLastRun.at), {
                        addSuffix: true,
                      })}
                      : {markdownExportLastRun.detail}
                    </Trans>
                  ) : (
                    <Trans>
                      Last run failed{" "}
                      {formatDistanceToNow(new Date(markdownExportLastRun.at), {
                        addSuffix: true,
                      })}
                      : {markdownExportLastRun.detail}
                    </Trans>
                  )}
                </p>
              ) : null}
            </div>
          ) : null}

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
