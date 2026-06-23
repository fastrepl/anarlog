import { useForm } from "@tanstack/react-form";
import { HeartIcon, MoreHorizontalIcon, Plus, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";
import { Input } from "@hypr/ui/components/ui/input";
import { Textarea } from "@hypr/ui/components/ui/textarea";
import { cn } from "@hypr/utils";

import {
  type UserTemplate,
  useSaveTemplate,
  useToggleTemplateFavorite,
} from "./queries";
import { SectionsList } from "./sections-editor";

import { TemplateCategoryLabel } from "~/shared/ui/template-category-label";
import * as settings from "~/store/tinybase/store/settings";

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
      className="mt-2 flex min-h-6 w-full cursor-text flex-wrap items-center gap-1.5"
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
          className="bg-muted flex h-6 items-center gap-1 rounded-md px-2 py-0.5 text-xs font-normal"
        >
          {target}
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="ml-0.5 h-3 w-3 p-0 hover:bg-transparent"
            onClick={(e) => {
              e.stopPropagation();
              onChange(
                value.filter((_, currentIndex) => currentIndex !== index),
              );
            }}
          >
            <X className="h-2.5 w-2.5" />
          </Button>
        </Badge>
      ))}

      {!isAddingTag ? (
        <button
          type="button"
          className="bg-muted text-muted-foreground hover:bg-muted/80 inline-flex h-6 items-center gap-1 rounded-md px-2 py-0.5 text-xs transition-colors"
          onClick={() => setIsAddingTag(true)}
        >
          <Plus className="h-3 w-3" />
          Add tag
        </button>
      ) : (
        <input
          ref={inputRef}
          type="text"
          autoFocus
          value={inputValue}
          className="text-muted-foreground min-w-[84px] flex-1 bg-transparent py-0 text-xs leading-none outline-hidden"
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
  const { id } = template;
  const saveTemplate = useSaveTemplate();
  const toggleTemplateFavorite = useToggleTemplateFavorite();
  const [actionsOpen, setActionsOpen] = useState(false);
  const didInitializeForm = useRef(false);

  const selectedTemplateId = settings.UI.useValue(
    "selected_template_id",
    settings.STORE_ID,
  ) as string | undefined;
  const isDefault = selectedTemplateId === id;

  const setSelectedTemplateId = settings.UI.useSetValueCallback(
    "selected_template_id",
    () => (isDefault ? "" : id),
    [id, isDefault],
    settings.STORE_ID,
  );

  const form = useForm({
    defaultValues: {
      title: template.title ?? "",
      description: template.description ?? "",
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

  useEffect(() => {
    didInitializeForm.current = false;
  }, [id]);

  useEffect(() => {
    if (didInitializeForm.current) {
      return;
    }

    form.reset({
      title: template.title ?? "",
      description: template.description ?? "",
      targets: template.targets ?? [],
      sections: template.sections ?? [],
    });
    didInitializeForm.current = true;
  }, [form, template]);

  return (
    <div className="flex h-full flex-1 flex-col">
      <div className="flex h-12 items-center justify-between gap-3 pr-1 pl-3">
        <div className="min-w-0">
          <TemplateCategoryLabel category={template.category} />
        </div>
        <div className="flex items-center gap-0">
          <Button
            type="button"
            size="sm"
            variant="ghost"
            onClick={setSelectedTemplateId}
            title={isDefault ? "Remove as default" : "Set as default"}
            className={cn([
              "text-muted-foreground shrink-0 hover:text-black",
              isDefault ? "bg-muted hover:bg-accent text-black" : null,
            ])}
          >
            {isDefault ? "Current default" : "Set as default"}
          </Button>
          <Button
            type="button"
            size="icon"
            variant="ghost"
            onClick={() => toggleTemplateFavorite(id)}
            className={cn([
              "text-muted-foreground hover:text-foreground",
              template.pinned && "text-rose-500 hover:text-rose-600",
            ])}
            title={
              template.pinned ? "Unfavorite template" : "Favorite template"
            }
            aria-label={
              template.pinned ? "Unfavorite template" : "Favorite template"
            }
          >
            <HeartIcon
              className="size-4"
              fill={template.pinned ? "currentColor" : "none"}
            />
          </Button>
          <DropdownMenu open={actionsOpen} onOpenChange={setActionsOpen}>
            <DropdownMenuTrigger asChild>
              <Button
                type="button"
                size="icon"
                variant="ghost"
                className={cn([
                  "text-muted-foreground hover:text-foreground",
                  actionsOpen && "bg-muted text-foreground hover:bg-accent",
                ])}
                aria-label="Template actions"
              >
                <MoreHorizontalIcon className="size-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent variant="app" align="end">
              <AppFloatingPanel className="overflow-hidden p-1">
                <DropdownMenuItem
                  onClick={() => handleDuplicateTemplate(id)}
                  className="cursor-pointer"
                >
                  Duplicate
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => handleDeleteTemplate(id)}
                  className="cursor-pointer text-red-600 focus:text-red-600"
                >
                  Delete
                </DropdownMenuItem>
              </AppFloatingPanel>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>

      <div className="relative min-h-0 flex-1 overflow-hidden">
        <div className="scroll-fade-y h-full overflow-y-auto px-6 pt-3 pb-6">
          <div className="min-w-0">
            <form.Field name="title">
              {(field) => (
                <div className="flex min-w-0 items-baseline gap-2">
                  <div className="relative max-w-full min-w-0">
                    <span
                      aria-hidden="true"
                      className="invisible block px-0 py-0 text-lg font-semibold whitespace-pre md:text-lg"
                    >
                      {(field.state.value || " ") + " "}
                    </span>
                    <Input
                      value={field.state.value}
                      onChange={(e) => field.handleChange(e.target.value)}
                      placeholder="Enter template title"
                      className="absolute inset-0 h-auto w-full max-w-full min-w-0 border-0 px-0 py-0 text-lg font-semibold shadow-none focus-visible:ring-0 md:text-lg"
                    />
                  </div>
                </div>
              )}
            </form.Field>
            <form.Field name="description">
              {(field) => (
                <Textarea
                  value={field.state.value}
                  onChange={(e) => field.handleChange(e.target.value)}
                  placeholder="Describe the template purpose..."
                  className="text-muted-foreground mt-1 min-h-[24px] resize-none border-0 px-0 py-0 text-sm shadow-none focus-visible:ring-0"
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
              <div className="mt-6">
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
