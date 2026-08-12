import { Trans, useLingui } from "@lingui/react/macro";
import {
  CircleNotch,
  LockSimple,
  MagicWand,
  Sparkle,
} from "@phosphor-icons/react";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useRef, useState } from "react";

import { PromptEditor, type PromptEditorHandle } from "@anlg/editor/prompt";
import { commands as templateCommands } from "@anlg/plugin-template";
import { Badge } from "@anlg/ui/components/ui/badge";
import { Button } from "@anlg/ui/components/ui/button";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

import { AutoFormatExamplesDialog } from "./auto-format-examples-dialog";

import { useBillingAccess } from "~/auth/billing-context";
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
        billing.upgradeToPro();
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
      billing.upgradeToPro();
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
          <Badge variant="secondary" className="h-5 rounded-full text-[10px]">
            {isCustomized ? <Trans>Customized</Trans> : <Trans>Default</Trans>}
          </Badge>
        </div>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          className={cn([
            "text-muted-foreground shrink-0 hover:text-black",
            isDefault ? "bg-muted hover:bg-accent text-black" : null,
          ])}
          onClick={() => {
            void setSettingValue("selected_template_id", "").catch((error) => {
              console.error("[templates] failed to set Auto as default", error);
            });
          }}
          disabled={isDefault}
        >
          {isDefault ? (
            <Trans>Current default</Trans>
          ) : (
            <Trans>Set as default</Trans>
          )}
        </Button>
      </div>

      <div className="scroll-fade-y min-h-0 flex-1 overflow-y-auto px-6 pt-3 pb-6">
        <div className="flex max-w-4xl flex-col gap-5">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h1 className="text-lg font-semibold">
                <Trans>Summary format</Trans>
              </h1>
              <p className="text-muted-foreground mt-1 text-sm">
                {billing.isPro ? (
                  <Trans>
                    Choose how Auto structures and styles your summaries.
                  </Trans>
                ) : (
                  <Trans>
                    Preview the summary format, then upgrade to Pro to customize
                    it.
                  </Trans>
                )}
              </p>
            </div>
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="shrink-0"
              onClick={() => {
                if (!billing.isPro) {
                  billing.upgradeToPro();
                  return;
                }
                setShowExamplesDialog(true);
              }}
              disabled={billing.isUpgradingToPro}
            >
              {billing.isPro ? (
                <MagicWand className="size-4" />
              ) : (
                <LockSimple className="size-4" />
              )}
              <Trans>Improve with examples</Trans>
            </Button>
          </div>

          <form.Field name="format">
            {(field) => (
              <div className="border-border bg-card overflow-hidden rounded-2xl border">
                <div className="group/editor relative">
                  <PromptEditor
                    ref={editorRef}
                    ariaLabel={t`Auto summary format`}
                    className="min-h-[28rem] px-4 py-3 font-mono text-sm leading-5"
                    initialValue={field.state.value}
                    maxLength={16000}
                    onChange={field.handleChange}
                    onBlur={field.handleBlur}
                    readOnly={!billing.isPro}
                    tokens={AUTO_FORMAT_TOKENS}
                  />
                  {!billing.isPro ? (
                    <button
                      type="button"
                      onClick={billing.upgradeToPro}
                      disabled={billing.isUpgradingToPro}
                      aria-label={t`Upgrade to Pro to customize Auto format`}
                      className="focus-visible:ring-ring absolute inset-0 cursor-pointer rounded-2xl focus-visible:ring-2 focus-visible:outline-none"
                    >
                      <span className="border-primary bg-primary text-primary-foreground pointer-events-none absolute top-3 right-3 flex translate-x-1 items-center gap-1 rounded-full border-2 px-3 py-1 text-xs font-medium opacity-0 shadow-[0_4px_14px_rgba(87,83,78,0.18)] transition-all duration-150 group-focus-within/editor:translate-x-0 group-focus-within/editor:opacity-100 group-hover/editor:translate-x-0 group-hover/editor:opacity-100">
                        {billing.isUpgradingToPro ? (
                          <CircleNotch
                            className="size-3 animate-spin"
                            aria-hidden
                          />
                        ) : (
                          <LockSimple className="size-3" aria-hidden />
                        )}
                        <Trans>Upgrade to customize</Trans>
                      </span>
                    </button>
                  ) : null}
                </div>
              </div>
            )}
          </form.Field>

          <div className="flex items-center justify-end gap-2">
            <form.Subscribe selector={(state) => state.values.format}>
              {(currentFormat) => (
                <Button
                  type="button"
                  variant="ghost"
                  disabled={
                    !billing.isPro ||
                    (!isCustomized &&
                      formatsMatch(currentFormat, defaultFormat)) ||
                    saveMutation.isPending
                  }
                  onClick={() => {
                    void resetToDefault().catch(() => {});
                  }}
                >
                  <Trans>Reset to default format</Trans>
                </Button>
              )}
            </form.Subscribe>
            {billing.isPro ? (
              <form.Subscribe
                selector={(state) => [state.canSubmit, state.isDirty] as const}
              >
                {([canSubmit, isDirty]) => (
                  <Button
                    type="submit"
                    disabled={!canSubmit || !isDirty || saveMutation.isPending}
                  >
                    <Trans>Save</Trans>
                  </Button>
                )}
              </form.Subscribe>
            ) : (
              <Button
                type="button"
                onClick={billing.upgradeToPro}
                disabled={billing.isUpgradingToPro}
              >
                <LockSimple className="size-4" />
                <Trans>Get Pro to customize</Trans>
              </Button>
            )}
          </div>
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
