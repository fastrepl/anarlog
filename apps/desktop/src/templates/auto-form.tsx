import { Trans, useLingui } from "@lingui/react/macro";
import { Check, DotsThree, MagicWand, Sparkle } from "@phosphor-icons/react";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useRef, useState } from "react";

import { PromptEditor, type PromptEditorHandle } from "@anlg/editor/prompt";
import { commands as templateCommands } from "@anlg/plugin-template";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  appFloatingMenuPanelClassName,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

import { AutoFormatExamplesDialog } from "./auto-format-examples-dialog";

import { useBillingAccess } from "~/auth/billing-context";
import { PlanGate, useNotifyPlanRequired } from "~/settings/plan-gate";
import { setSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";

const AUTO_FORMAT_TOKENS = [] as const;
const LEGACY_CUSTOM_INSTRUCTIONS_PREAMBLE =
  "For structure, formatting, tone, and emphasis, these instructions take precedence over the Format Requirements. They do not override the requirements to stay accurate, use only the provided source material, and return only the summary.";

export function AutoTemplateDetails() {
  const formatOverride = useConfigValue("auto_summary_prompt");
  const sourceQuery = useQuery({
    queryKey: ["template-source", "enhance-format"],
    queryFn: loadDefaultAutoFormat,
    staleTime: Number.POSITIVE_INFINITY,
  });

  if (sourceQuery.isLoading) {
    return (
      <div className="text-muted-foreground flex h-full items-center justify-center text-sm">
        <Trans>Loading Auto format...</Trans>
      </div>
    );
  }

  if (sourceQuery.error || !sourceQuery.data) {
    return (
      <div className="text-destructive flex h-full items-center justify-center px-6 text-center text-sm">
        {sourceQuery.error?.message || "Auto format is unavailable."}
      </div>
    );
  }

  return (
    <AutoFormatForm
      key={`${formatOverride}:${sourceQuery.data}`}
      defaultFormat={sourceQuery.data}
      formatOverride={formatOverride}
    />
  );
}

export function AutoFormatForm({
  defaultFormat,
  formatOverride,
}: {
  defaultFormat: string;
  formatOverride: string;
}) {
  const { t } = useLingui();
  const billing = useBillingAccess();
  const notifyPlanRequired = useNotifyPlanRequired();
  const editorRef = useRef<PromptEditorHandle>(null);
  const [showExamplesDialog, setShowExamplesDialog] = useState(false);
  const selectedTemplateId = useConfigValue("selected_template_id");
  const isDefault = !selectedTemplateId;
  const normalizedOverride = normalizeFormatOverride(formatOverride);
  const isCustomized =
    Boolean(normalizedOverride) &&
    !formatsMatch(normalizedOverride, defaultFormat);
  const initialFormat = isCustomized ? normalizedOverride : defaultFormat;

  const saveMutation = useMutation({
    mutationFn: async (source: string) => {
      const normalized = normalizeFormat(source);
      if (!normalized) {
        throw new Error(t`Summary format cannot be empty.`);
      }
      const stored = formatsMatch(normalized, defaultFormat) ? "" : normalized;
      const rendered = await templateCommands.render({
        enhanceSystem: {
          language: "en",
          formatOverride: stored,
        },
      });
      if (rendered.status === "error") {
        throw new Error(rendered.error);
      }

      await setSettingValue("auto_summary_prompt", stored);
      return stored;
    },
    onError: (error) => sonnerToast.error(error.message),
  });

  const form = useForm({
    defaultValues: { format: initialFormat },
    onSubmit: async ({ value }) => {
      if (!billing.isPro) {
        notifyPlanRequired("pro");
        return;
      }

      const stored = await saveMutation.mutateAsync(value.format);
      const nextFormat = stored || defaultFormat;
      form.reset({ format: nextFormat });
      editorRef.current?.setValue(nextFormat);
    },
  });

  const resetToDefault = async () => {
    if (!billing.isPro) {
      notifyPlanRequired("pro");
      return;
    }

    await saveMutation.mutateAsync(defaultFormat);
    form.reset({ format: defaultFormat });
    editorRef.current?.setValue(defaultFormat);
  };

  return (
    <form
      className="flex h-full min-h-0 flex-col"
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit().catch(() => {});
      }}
    >
      <div className="flex h-12 shrink-0 items-center justify-between gap-3 pr-1 pl-3">
        <div className="flex min-w-0 items-center gap-2">
          <Sparkle className="size-4 shrink-0 text-violet-500" />
          <span className="truncate text-sm font-semibold">Auto</span>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <Button
            type="button"
            size="sm"
            variant="ghost"
            className={cn([
              "text-muted-foreground shrink-0 hover:text-black",
              isDefault
                ? "text-emerald-600 hover:bg-transparent hover:text-emerald-700 disabled:opacity-100 dark:text-emerald-400 dark:hover:text-emerald-300"
                : null,
            ])}
            onClick={() => {
              void setSettingValue("selected_template_id", "").catch(
                (error) => {
                  console.error(
                    "[templates] failed to set Auto as default",
                    error,
                  );
                },
              );
            }}
            disabled={isDefault}
          >
            {isDefault ? (
              <>
                <Check className="size-3.5" weight="bold" />
                <Trans>Current default</Trans>
              </>
            ) : (
              <Trans>Set as default</Trans>
            )}
          </Button>
          <form.Subscribe selector={(state) => state.values.format}>
            {(currentFormat) => (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    aria-label={t`Template actions`}
                    className="text-muted-foreground hover:bg-accent hover:text-foreground rounded-full"
                  >
                    <DotsThree size={16} />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent variant="app" align="end" className="w-56">
                  <AppFloatingPanel className={appFloatingMenuPanelClassName}>
                    <DropdownMenuItem
                      className="cursor-pointer"
                      disabled={
                        (!isCustomized &&
                          formatsMatch(currentFormat, defaultFormat)) ||
                        saveMutation.isPending
                      }
                      onClick={() => {
                        void resetToDefault().catch(() => {});
                      }}
                    >
                      <Trans>Reset to default format</Trans>
                    </DropdownMenuItem>
                  </AppFloatingPanel>
                </DropdownMenuContent>
              </DropdownMenu>
            )}
          </form.Subscribe>
        </div>
      </div>

      <div className="scroll-fade-y min-h-0 flex-1 overflow-y-auto px-6 pt-3 pb-6">
        <div className="flex max-w-4xl flex-col gap-5">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h1 className="text-lg font-semibold">
                <Trans>Summary format</Trans>
              </h1>
              <p className="text-muted-foreground mt-1 text-sm">
                <Trans>
                  Choose how Auto structures and styles your summaries.
                </Trans>
              </p>
            </div>
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="shrink-0"
              onClick={() => {
                if (!billing.isPro) {
                  notifyPlanRequired("pro");
                  return;
                }
                setShowExamplesDialog(true);
              }}
            >
              <MagicWand className="size-4" />
              <Trans>Improve with examples</Trans>
            </Button>
          </div>

          <PlanGate plan="pro" allowed={billing.isPro}>
            <div className="flex flex-col gap-5">
              <form.Field name="format">
                {(field) => (
                  <div className="border-border bg-card overflow-hidden rounded-2xl border">
                    <PromptEditor
                      ref={editorRef}
                      ariaLabel={t`Auto summary format`}
                      className="min-h-[28rem] px-4 py-3 font-mono text-sm leading-5"
                      initialValue={field.state.value}
                      maxLength={16000}
                      onChange={field.handleChange}
                      onBlur={field.handleBlur}
                      tokens={AUTO_FORMAT_TOKENS}
                    />
                  </div>
                )}
              </form.Field>

              <div className="flex items-center justify-end gap-2">
                <form.Subscribe
                  selector={(state) =>
                    [state.canSubmit, state.isDirty] as const
                  }
                >
                  {([canSubmit, isDirty]) => (
                    <Button
                      type="submit"
                      disabled={
                        !canSubmit || !isDirty || saveMutation.isPending
                      }
                    >
                      <Trans>Save</Trans>
                    </Button>
                  )}
                </form.Subscribe>
              </div>
            </div>
          </PlanGate>
        </div>
      </div>

      {showExamplesDialog ? (
        <AutoFormatExamplesDialog
          onClose={() => setShowExamplesDialog(false)}
          onGenerated={(format) => {
            form.setFieldValue("format", format);
            editorRef.current?.setValue(format);
          }}
        />
      ) : null}
    </form>
  );
}

async function loadDefaultAutoFormat(): Promise<string> {
  const result = await templateCommands.getTemplateSource("enhanceFormat");
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

function normalizeFormat(value: string): string {
  return value.replace(/\r\n/g, "\n").trim();
}

function formatsMatch(a: string, b: string): boolean {
  return normalizeFormat(a) === normalizeFormat(b);
}

function normalizeFormatOverride(value: string): string {
  const normalized = normalizeFormat(value);
  if (
    !normalized.includes("# General Instructions") ||
    !normalized.includes("# About Notes")
  ) {
    return normalized;
  }

  const formatRequirements = headingSection(
    normalized,
    "# Format Requirements",
  );
  if (formatRequirements === null) {
    return normalized;
  }

  const customInstructions = headingSection(
    normalized,
    "# Custom Summary Instructions",
  )
    ?.replace(LEGACY_CUSTOM_INSTRUCTIONS_PREAMBLE, "")
    .trim();

  return [formatRequirements, customInstructions].filter(Boolean).join("\n\n");
}

function headingSection(source: string, heading: string): string | null {
  const lines = source.split("\n");
  const headingIndex = lines.findIndex((line) => line.trim() === heading);
  if (headingIndex === -1) return null;

  const followingLines = lines.slice(headingIndex + 1);
  const nextHeadingIndex = followingLines.findIndex((line) =>
    line.trimStart().startsWith("# "),
  );

  return followingLines
    .slice(0, nextHeadingIndex === -1 ? undefined : nextHeadingIndex)
    .join("\n")
    .trim();
}
