import { Trans, useLingui } from "@lingui/react/macro";
import {
  BracketsCurly,
  CircleNotch,
  LockSimple,
  Sparkle,
} from "@phosphor-icons/react";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useRef } from "react";

import {
  PromptEditor,
  type PromptEditorHandle,
  type PromptTokenDefinition,
} from "@anlg/editor/prompt";
import { commands as templateCommands } from "@anlg/plugin-template";
import { Badge } from "@anlg/ui/components/ui/badge";
import { Button } from "@anlg/ui/components/ui/button";
import { cn } from "@anlg/utils";

import { useBillingAccess } from "~/auth/billing-context";
import { setSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";

export const AUTO_PROMPT_TOKENS: readonly PromptTokenDefinition[] = [
  { name: "current_date", label: "Current date" },
  { name: "language", label: "Language" },
];

export function AutoTemplateDetails() {
  const promptOverride = useConfigValue("auto_summary_prompt");
  const sourceQuery = useQuery({
    queryKey: ["template-source", "enhance-system"],
    queryFn: loadDefaultAutoPrompt,
    staleTime: Number.POSITIVE_INFINITY,
  });

  if (sourceQuery.isLoading) {
    return (
      <div className="text-muted-foreground flex h-full items-center justify-center text-sm">
        <Trans>Loading Auto prompt...</Trans>
      </div>
    );
  }

  if (sourceQuery.error || !sourceQuery.data) {
    return (
      <div className="text-destructive flex h-full items-center justify-center px-6 text-center text-sm">
        {sourceQuery.error?.message || "Auto prompt is unavailable."}
      </div>
    );
  }

  return (
    <AutoPromptForm
      key={`${promptOverride}:${sourceQuery.data}`}
      defaultPrompt={sourceQuery.data}
      promptOverride={promptOverride}
    />
  );
}

export function AutoPromptForm({
  defaultPrompt,
  promptOverride,
}: {
  defaultPrompt: string;
  promptOverride: string;
}) {
  const { t } = useLingui();
  const billing = useBillingAccess();
  const editorRef = useRef<PromptEditorHandle>(null);
  const selectedTemplateId = useConfigValue("selected_template_id");
  const isDefault = !selectedTemplateId;
  const isCustomized = Boolean(promptOverride.trim());
  const initialPrompt = isCustomized ? promptOverride : defaultPrompt;

  const saveMutation = useMutation({
    mutationFn: async (source: string) => {
      const normalized = normalizePrompt(source);
      if (!normalized) {
        throw new Error(t`Auto prompt cannot be empty.`);
      }
      const stored = promptsMatch(normalized, defaultPrompt) ? "" : normalized;
      const rendered = await templateCommands.render({
        enhanceSystem: {
          language: "en",
          promptOverride: stored,
        },
      });
      if (rendered.status === "error") {
        throw new Error(rendered.error);
      }

      await setSettingValue("auto_summary_prompt", stored);
      return stored;
    },
  });

  const form = useForm({
    defaultValues: { prompt: initialPrompt },
    onSubmit: async ({ value }) => {
      if (!billing.isPro) {
        billing.upgradeToPro();
        return;
      }

      const stored = await saveMutation.mutateAsync(value.prompt);
      const nextPrompt = stored || defaultPrompt;
      form.reset({ prompt: nextPrompt });
      editorRef.current?.setValue(nextPrompt);
    },
  });

  const resetToDefault = async () => {
    if (!billing.isPro) {
      billing.upgradeToPro();
      return;
    }

    await saveMutation.mutateAsync(defaultPrompt);
    form.reset({ prompt: defaultPrompt });
    editorRef.current?.setValue(defaultPrompt);
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
        <div className="mx-auto flex max-w-4xl flex-col gap-5">
          <div>
            <h1 className="text-lg font-semibold">
              <Trans>Customize Auto</Trans>
            </h1>
            <p className="text-muted-foreground mt-1 text-sm">
              {billing.isPro ? (
                <Trans>
                  Edit the complete system prompt used when Auto generates a
                  summary.
                </Trans>
              ) : (
                <Trans>
                  Preview the complete system prompt, then upgrade to Pro to
                  customize it.
                </Trans>
              )}
            </p>
          </div>

          <form.Field name="prompt">
            {(field) => (
              <div className="border-border bg-card overflow-hidden rounded-2xl border">
                <div className="group/editor relative">
                  <PromptEditor
                    ref={editorRef}
                    ariaLabel={t`Auto summary prompt`}
                    className="min-h-[28rem] px-4 py-3 font-mono text-sm leading-5"
                    initialValue={field.state.value}
                    maxLength={16000}
                    onChange={field.handleChange}
                    onBlur={field.handleBlur}
                    readOnly={!billing.isPro}
                    tokens={AUTO_PROMPT_TOKENS}
                  />
                  {!billing.isPro ? (
                    <button
                      type="button"
                      onClick={billing.upgradeToPro}
                      disabled={billing.isUpgradingToPro}
                      aria-label={t`Upgrade to Pro to customize Auto`}
                      className="focus-visible:ring-ring absolute inset-0 cursor-pointer rounded-t-2xl focus-visible:ring-2 focus-visible:outline-none"
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
                <div className="border-border bg-muted/40 flex items-center justify-between gap-3 border-t px-3 py-2">
                  <span className="text-muted-foreground text-xs font-medium">
                    <Trans>Variables</Trans>
                  </span>
                  <div className="flex flex-wrap justify-end gap-1.5">
                    {AUTO_PROMPT_TOKENS.map((token) => (
                      <Button
                        key={token.name}
                        type="button"
                        variant="outline"
                        size="sm"
                        className="h-7 rounded-full px-2.5 text-xs"
                        onMouseDown={(event) => event.preventDefault()}
                        onClick={() => {
                          if (!billing.isPro) {
                            billing.upgradeToPro();
                            return;
                          }
                          editorRef.current?.insertToken(token.name);
                        }}
                      >
                        <BracketsCurly className="size-3.5" />
                        {token.label}
                      </Button>
                    ))}
                  </div>
                </div>
              </div>
            )}
          </form.Field>

          <div className="rounded-2xl border px-4 py-3">
            <p className="text-sm font-medium">
              <Trans>Context always provided</Trans>
            </p>
            <p className="text-muted-foreground mt-1 text-sm">
              <Trans>
                Anarlog sends these separately, so editing the prompt cannot
                remove the meeting source material.
              </Trans>
            </p>
            <div className="mt-3 flex flex-wrap gap-2">
              <Badge variant="secondary" className="rounded-full">
                <Trans>Meeting notes</Trans>
              </Badge>
              <Badge variant="secondary" className="rounded-full">
                <Trans>Transcript</Trans>
              </Badge>
              <Badge variant="secondary" className="rounded-full">
                <Trans>Session details</Trans>
              </Badge>
              <Badge variant="secondary" className="rounded-full">
                <Trans>Participants</Trans>
              </Badge>
            </div>
          </div>

          {saveMutation.error ? (
            <p role="alert" className="text-destructive text-sm">
              {saveMutation.error.message}
            </p>
          ) : null}

          <div className="flex items-center justify-end gap-2">
            <form.Subscribe selector={(state) => state.values.prompt}>
              {(currentPrompt) => (
                <Button
                  type="button"
                  variant="ghost"
                  disabled={
                    !billing.isPro ||
                    (!isCustomized &&
                      promptsMatch(currentPrompt, defaultPrompt)) ||
                    saveMutation.isPending
                  }
                  onClick={() => {
                    void resetToDefault().catch(() => {});
                  }}
                >
                  <Trans>Reset to Anarlog default</Trans>
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
    </form>
  );
}

async function loadDefaultAutoPrompt(): Promise<string> {
  const result = await templateCommands.getTemplateSource("enhanceSystem");
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

function normalizePrompt(value: string): string {
  return value.replace(/\r\n/g, "\n").trim();
}

function promptsMatch(a: string, b: string): boolean {
  return normalizePrompt(a) === normalizePrompt(b);
}
