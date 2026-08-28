import { Trans, useLingui } from "@lingui/react/macro";
import { Check, DotsThree, Heart, Plus, X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useForm } from "@tanstack/react-form";
import { useRef, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Badge } from "@anlg/ui/components/ui/badge";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { Input } from "@anlg/ui/components/ui/input";
import { Textarea } from "@anlg/ui/components/ui/textarea";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import {
  type UserTemplate,
  useSaveTemplate,
  useToggleTemplateFavorite,
} from "./queries";
import { SectionsList } from "./sections-editor";
import { TemplateIconPicker } from "./template-icon-picker";

import { useSetSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";

function parseTargets(value: string) {
  return value
    .split(",")
    .map((target) => target.trim())
    .filter(Boolean);
}

function TemplateTargetsInput({
  value,
  onChange,
}: {
  value: string[];
  onChange: (value: string[]) => void;
}) {
  const [inputValue, setInputValue] = useState("");
  const [isAddingTag, setIsAddingTag] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const submitTargets = () => {
    const nextTargets = parseTargets(inputValue);
    if (nextTargets.length === 0) {
      setInputValue("");
      setIsAddingTag(false);
      return;
    }

    onChange([...value, ...nextTargets]);
    setInputValue("");
    setIsAddingTag(false);
  };

  return (
    <div
      {...stylex.props(styles.targets)}
      onClick={() => {
        if (!isAddingTag) {
          setIsAddingTag(true);
          return;
        }

        inputRef.current?.focus();
      }}
    >
      {value.map((target, index) => (
        <Badge
          key={`${target}-${index}`}
          variant="secondary"
          sx={styles.targetBadge}
        >
          {target}
          <Button
            type="button"
            variant="ghost"
            size="sm"
            sx={styles.removeTargetButton}
            onClick={(e) => {
              e.stopPropagation();
              onChange(
                value.filter((_, currentIndex) => currentIndex !== index),
              );
            }}
          >
            <X {...stylex.props(styles.removeTargetIcon)} />
          </Button>
        </Badge>
      ))}

      {!isAddingTag ? (
        <button
          type="button"
          {...stylex.props(styles.addTargetButton)}
          onClick={() => setIsAddingTag(true)}
        >
          <Plus {...stylex.props(styles.smallIcon)} />
          Add tag
        </button>
      ) : (
        <input
          ref={inputRef}
          type="text"
          autoFocus
          value={inputValue}
          {...stylex.props(styles.targetInput)}
          onChange={(e) => setInputValue(e.target.value)}
          onBlur={submitTargets}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === "Tab" || e.key === ",") {
              if (!inputValue.trim()) {
                return;
              }

              e.preventDefault();
              submitTargets();
              return;
            }

            if (e.key === "Escape") {
              e.preventDefault();
              setInputValue("");
              setIsAddingTag(false);
              return;
            }

            if (e.key === "Backspace" && !inputValue && value.length > 0) {
              e.preventDefault();
              onChange(value.slice(0, -1));
            }
          }}
        />
      )}
    </div>
  );
}

export function TemplateForm({
  template,
  handleDeleteTemplate,
  handleDuplicateTemplate,
}: {
  template: UserTemplate;
  handleDeleteTemplate: (id: string) => void;
  handleDuplicateTemplate: (id: string) => void;
}) {
  const { t } = useLingui();
  const { id } = template;
  const saveTemplate = useSaveTemplate();
  const toggleTemplateFavorite = useToggleTemplateFavorite();
  const [actionsOpen, setActionsOpen] = useState(false);

  const selectedTemplateId = useConfigValue("selected_template_id");
  const isDefault = selectedTemplateId === id;

  const setDefaultTemplateId = useSetSettingValue("selected_template_id");
  const setSelectedTemplateId = () => {
    setDefaultTemplateId(isDefault ? "" : id);
  };

  const form = useForm({
    defaultValues: {
      title: template.title ?? "",
      description: template.description ?? "",
      icon: template.icon,
      targets: template.targets ?? [],
      sections: template.sections ?? [],
    },
    listeners: {
      onChange: ({ formApi }) => {
        queueMicrotask(() => {
          const {
            form: { errors },
          } = formApi.getAllErrors();
          if (errors.length === 0) {
            void formApi.handleSubmit();
          }
        });
      },
    },
    onSubmit: ({ value }) => {
      return saveTemplate({
        ...template,
        ...value,
      });
    },
  });

  return (
    <div {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.header)}>
        <div {...stylex.props(styles.headerIdentity)}>
          <form.Field name="icon">
            {(field) => (
              <TemplateIconPicker
                size="sm"
                value={field.state.value}
                onChange={field.handleChange}
              />
            )}
          </form.Field>
          <form.Field name="title">
            {(field) => (
              <div {...stylex.props(styles.titleField)}>
                <span aria-hidden="true" {...stylex.props(styles.titleMeasure)}>
                  {(field.state.value || t`Enter template title`) + " "}
                </span>
                <Input
                  value={field.state.value}
                  onChange={(e) => field.handleChange(e.target.value)}
                  placeholder={t`Enter template title`}
                  sx={styles.titleInput}
                />
              </div>
            )}
          </form.Field>
        </div>
        <div {...stylex.props(styles.headerActions)}>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            onClick={setSelectedTemplateId}
            aria-pressed={isDefault}
            title={isDefault ? "Remove as default" : "Set as default"}
            sx={[
              styles.defaultButton,
              isDefault && styles.currentDefaultButton,
            ]}
          >
            {isDefault ? (
              <>
                <Check {...stylex.props(styles.mediumIcon)} weight="bold" />
                Current default
              </>
            ) : (
              "Set as default"
            )}
          </Button>
          <Button
            type="button"
            size="icon"
            variant="ghost"
            onClick={() => toggleTemplateFavorite(id)}
            sx={[styles.iconButton, template.pinned && styles.favoriteButton]}
            title={
              template.pinned ? "Unfavorite template" : "Favorite template"
            }
            aria-label={
              template.pinned ? "Unfavorite template" : "Favorite template"
            }
          >
            <Heart
              {...stylex.props(styles.icon)}
              weight={template.pinned ? "fill" : "regular"}
            />
          </Button>
          <DropdownMenu open={actionsOpen} onOpenChange={setActionsOpen}>
            <DropdownMenuTrigger asChild>
              <Button
                type="button"
                size="icon"
                variant="ghost"
                sx={[styles.iconButton, actionsOpen && styles.openActionButton]}
                aria-label={t`Template actions`}
              >
                <DotsThree {...stylex.props(styles.icon)} />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent variant="app" align="end">
              <AppFloatingPanel sx={styles.menuPanel}>
                <DropdownMenuItem
                  onClick={() => handleDuplicateTemplate(id)}
                  sx={styles.menuItem}
                >
                  <Trans>Duplicate</Trans>
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => handleDeleteTemplate(id)}
                  sx={[styles.menuItem, styles.deleteItem]}
                >
                  <Trans>Delete</Trans>
                </DropdownMenuItem>
              </AppFloatingPanel>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>

      <div {...stylex.props(styles.body)}>
        <div {...mergeStyleXProps(styles.scroll, "scroll-fade-y")}>
          <div {...stylex.props(styles.metadata)}>
            <form.Field name="description">
              {(field) => (
                <Textarea
                  value={field.state.value}
                  onChange={(e) => field.handleChange(e.target.value)}
                  placeholder={t`Describe the template purpose...`}
                  sx={styles.descriptionInput}
                  rows={1}
                />
              )}
            </form.Field>
            <form.Field name="targets">
              {(field) => (
                <TemplateTargetsInput
                  value={field.state.value}
                  onChange={field.handleChange}
                />
              )}
            </form.Field>
          </div>

          <form.Field name="sections">
            {(field) => (
              <div {...stylex.props(styles.sections)}>
                <SectionsList
                  disabled={false}
                  items={field.state.value}
                  onChange={(items) => field.handleChange(items)}
                />
              </div>
            )}
          </form.Field>
        </div>
      </div>
    </div>
  );
}

const styles = stylex.create({
  addTargetButton: {
    alignItems: "center",
    backgroundColor: {
      default: colors.muted,
      ":hover": `color-mix(in srgb, ${colors.muted} 80%, transparent)`,
    },
    borderRadius: radii.md,
    color: colors.mutedForeground,
    display: "inline-flex",
    fontSize: "0.75rem",
    gap: "0.25rem",
    height: "1.5rem",
    lineHeight: "1rem",
    paddingBlock: "0.125rem",
    paddingInline: "0.5rem",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  body: {
    flex: "1",
    minHeight: 0,
    overflow: "hidden",
    position: "relative",
  },
  currentDefaultButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": "transparent",
    },
    color: {
      default: "rgb(5 150 105)",
      ":hover": "rgb(4 120 87)",
      ":is(.dark *)": "rgb(52 211 153)",
      ":is(.dark *):hover": "rgb(110 231 183)",
    },
  },
  defaultButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": "black",
    },
    flexShrink: 0,
  },
  deleteItem: {
    color: {
      default: "rgb(220 38 38)",
      ":focus": "rgb(220 38 38)",
    },
  },
  descriptionInput: {
    borderWidth: 0,
    boxShadow: {
      default: "none",
      ":focus-visible": "none",
    },
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    minHeight: "24px",
    padding: 0,
    resize: "none",
  },
  favoriteButton: {
    color: {
      default: "rgb(244 63 94)",
      ":hover": "rgb(225 29 72)",
    },
  },
  header: {
    alignItems: "center",
    display: "flex",
    gap: "0.75rem",
    height: "3rem",
    justifyContent: "space-between",
    paddingLeft: "0.75rem",
    paddingRight: "0.25rem",
  },
  headerActions: {
    alignItems: "center",
    display: "flex",
    gap: 0,
  },
  headerIdentity: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    gap: "0.5rem",
    minWidth: 0,
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  iconButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
  },
  mediumIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  menuItem: {
    cursor: "pointer",
  },
  menuPanel: {
    overflow: "hidden",
    padding: "0.25rem",
  },
  metadata: {
    minWidth: 0,
  },
  openActionButton: {
    backgroundColor: {
      default: colors.muted,
      ":hover": colors.accent,
    },
    color: colors.foreground,
  },
  removeTargetButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": "transparent",
    },
    height: "0.75rem",
    marginLeft: "0.125rem",
    padding: 0,
    width: "0.75rem",
  },
  removeTargetIcon: {
    height: "0.625rem",
    width: "0.625rem",
  },
  root: {
    display: "flex",
    flex: "1",
    flexDirection: "column",
    height: "100%",
  },
  scroll: {
    height: "100%",
    overflowY: "auto",
    paddingBottom: "1.5rem",
    paddingInline: "1.5rem",
    paddingTop: "0.75rem",
  },
  sections: {
    marginTop: "1.5rem",
  },
  smallIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  targetBadge: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderRadius: radii.md,
    display: "flex",
    fontSize: "0.75rem",
    fontWeight: 400,
    gap: "0.25rem",
    height: "1.5rem",
    lineHeight: "1rem",
    paddingBlock: "0.125rem",
    paddingInline: "0.5rem",
  },
  targetInput: {
    backgroundColor: "transparent",
    color: colors.mutedForeground,
    flex: "1",
    fontSize: "0.75rem",
    lineHeight: 1,
    minWidth: "84px",
    outline: "none",
    paddingBlock: 0,
  },
  targets: {
    alignItems: "center",
    cursor: "text",
    display: "flex",
    flexWrap: "wrap",
    gap: "0.375rem",
    marginTop: "0.5rem",
    minHeight: "1.5rem",
    width: "100%",
  },
  titleField: {
    maxWidth: "100%",
    minWidth: 0,
    position: "relative",
  },
  titleInput: {
    borderWidth: 0,
    boxShadow: {
      default: "none",
      ":focus-visible": "none",
    },
    fontSize: "0.875rem",
    fontWeight: 600,
    height: "auto",
    inset: 0,
    maxWidth: "100%",
    minWidth: 0,
    padding: 0,
    position: "absolute",
    width: "100%",
  },
  titleMeasure: {
    display: "block",
    fontSize: "0.875rem",
    fontWeight: 600,
    padding: 0,
    visibility: "hidden",
    whiteSpace: "pre",
  },
});
