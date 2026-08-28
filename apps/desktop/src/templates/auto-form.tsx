import { Trans, useLingui } from "@lingui/react/macro";
import {
  Check,
  CircleNotch,
  DotsThree,
  LockSimple,
  MagicWand,
  Sparkle,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useRef, useState } from "react";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
import { PromptEditor, type PromptEditorHandle } from "@anlg/editor/prompt";
import { commands as templateCommands } from "@anlg/plugin-template";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

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
      <div {...stylex.props(styles.status, styles.loadingStatus)}>
        <Trans>Loading Auto format...</Trans>
      </div>
    );
  }

  if (sourceQuery.error || !sourceQuery.data) {
    return (
      <div {...stylex.props(styles.status, styles.errorStatus)}>
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
      {...stylex.props(styles.form)}
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit().catch(() => {});
      }}
    >
      <div {...stylex.props(styles.header)}>
        <div {...stylex.props(styles.headerTitle)}>
          <Sparkle {...stylex.props(styles.sparkle)} />
          <span {...stylex.props(styles.truncatedLabel)}>Auto</span>
        </div>
        <div {...stylex.props(styles.headerActions)}>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            sx={[
              styles.defaultButton,
              isDefault && styles.currentDefaultButton,
            ]}
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
                <Check {...stylex.props(styles.smallIcon)} weight="bold" />
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
                    sx={styles.actionsButton}
                  >
                    <DotsThree size={16} />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent variant="app" align="end" sx={styles.menu}>
                  <AppFloatingPanel sx={styles.menuPanel}>
                    <DropdownMenuItem
                      sx={styles.menuItem}
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
                    </DropdownMenuItem>
                  </AppFloatingPanel>
                </DropdownMenuContent>
              </DropdownMenu>
            )}
          </form.Subscribe>
        </div>
      </div>

      <div {...mergeStyleXProps(styles.scroller, "scroll-fade-y")}>
        <div {...stylex.props(styles.content)}>
          <div {...stylex.props(styles.intro)}>
            <div>
              <h1 {...stylex.props(styles.heading)}>
                <Trans>Summary format</Trans>
              </h1>
              <p {...stylex.props(styles.description)}>
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
              sx={styles.shrink}
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
                <MagicWand {...stylex.props(styles.icon)} />
              ) : (
                <LockSimple {...stylex.props(styles.icon)} />
              )}
              <Trans>Improve with examples</Trans>
            </Button>
          </div>

          <form.Field name="format">
            {(field) => (
              <div {...stylex.props(styles.editorFrame)}>
                <div
                  data-auto-format-editor
                  {...stylex.props(styles.editorSlot)}
                >
                  <PromptEditor
                    ref={editorRef}
                    ariaLabel={t`Auto summary format`}
                    sx={styles.editor}
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
                      {...stylex.props(styles.upgradeOverlay)}
                    >
                      <span {...stylex.props(styles.upgradeBadge)}>
                        {billing.isUpgradingToPro ? (
                          <CircleNotch
                            {...stylex.props(styles.upgradeSpinner)}
                            aria-hidden
                          />
                        ) : (
                          <LockSimple
                            {...stylex.props(styles.tinyIcon)}
                            aria-hidden
                          />
                        )}
                        <Trans>Upgrade to customize</Trans>
                      </span>
                    </button>
                  ) : null}
                </div>
              </div>
            )}
          </form.Field>

          <div {...stylex.props(styles.footer)}>
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
                <LockSimple {...stylex.props(styles.icon)} />
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

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  actionsButton: {
    backgroundColor: {
      default: null,
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
  },
  content: {
    display: "flex",
    flexDirection: "column",
    gap: "1.25rem",
    maxWidth: "56rem",
  },
  currentDefaultButton: {
    backgroundColor: {
      default: null,
      ":hover": "transparent",
    },
    color: {
      default: "rgb(5 150 105)",
      ":hover": "rgb(4 120 87)",
      ":is(.dark *)": "rgb(52 211 153)",
      ":is(.dark *):hover": "rgb(110 231 183)",
    },
    opacity: {
      default: 1,
      ":disabled": 1,
    },
  },
  defaultButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": "black",
    },
    flexShrink: 0,
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    marginTop: "0.25rem",
  },
  editor: {
    fontFamily: fonts.mono,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    minHeight: "28rem",
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  editorFrame: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    overflow: "hidden",
  },
  editorSlot: {
    position: "relative",
  },
  errorStatus: {
    color: colors.destructive,
    paddingInline: "1.5rem",
    textAlign: "center",
  },
  footer: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    justifyContent: "flex-end",
  },
  form: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    minHeight: 0,
  },
  header: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    gap: "0.75rem",
    height: "3rem",
    justifyContent: "space-between",
    paddingLeft: "0.75rem",
    paddingRight: "0.25rem",
  },
  headerActions: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    gap: "0.25rem",
  },
  headerTitle: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    minWidth: 0,
  },
  heading: {
    fontSize: "1.125rem",
    fontWeight: 600,
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  intro: {
    alignItems: "flex-start",
    display: "flex",
    gap: "1rem",
    justifyContent: "space-between",
  },
  loadingStatus: {
    color: colors.mutedForeground,
  },
  menu: {
    width: "14rem",
  },
  menuItem: {
    cursor: "pointer",
  },
  menuPanel: {
    overflow: "hidden",
    padding: "0.25rem",
  },
  scroller: {
    flex: "1",
    minHeight: 0,
    overflowY: "auto",
    paddingBottom: "1.5rem",
    paddingInline: "1.5rem",
    paddingTop: "0.75rem",
  },
  shrink: {
    flexShrink: 0,
  },
  smallIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  sparkle: {
    color: "rgb(139 92 246)",
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  status: {
    alignItems: "center",
    display: "flex",
    fontSize: "0.875rem",
    height: "100%",
    justifyContent: "center",
  },
  tinyIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  truncatedLabel: {
    fontSize: "0.875rem",
    fontWeight: 600,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  upgradeBadge: {
    alignItems: "center",
    backgroundColor: colors.primary,
    borderColor: colors.primary,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "2px",
    boxShadow: "0 4px 14px rgb(87 83 78 / 0.18)",
    color: colors.primaryForeground,
    display: "flex",
    fontSize: "0.75rem",
    fontWeight: 500,
    gap: "0.25rem",
    opacity: {
      default: 0,
      ":is([data-auto-format-editor]:focus-within *)": 1,
      ":is([data-auto-format-editor]:hover *)": 1,
    },
    paddingBlock: "0.25rem",
    paddingInline: "0.75rem",
    pointerEvents: "none",
    position: "absolute",
    right: "0.75rem",
    top: "0.75rem",
    transform: {
      default: "translateX(0.25rem)",
      ":is([data-auto-format-editor]:focus-within *)": "translateX(0)",
      ":is([data-auto-format-editor]:hover *)": "translateX(0)",
    },
    transitionDuration: "150ms",
    transitionProperty: "all",
  },
  upgradeOverlay: {
    borderRadius: "1rem",
    boxShadow: {
      default: null,
      ":focus-visible": `inset 0 0 0 2px ${colors.ring}`,
    },
    cursor: "pointer",
    inset: 0,
    outline: {
      default: null,
      ":focus-visible": "none",
    },
    position: "absolute",
  },
  upgradeSpinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "0.75rem",
    width: "0.75rem",
  },
});
