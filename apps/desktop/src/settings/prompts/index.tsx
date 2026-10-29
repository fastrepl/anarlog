import { Trans, useLingui } from "@lingui/react/macro";
import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";
import { useId, useState } from "react";

import {
  ChatCenteredDots,
  MinusCircle,
  PencilSimple,
  Plus,
} from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";
import { Textarea } from "@anlg/ui/components/ui/textarea";

import {
  type CustomChatPrompt,
  type CustomChatPromptAvailability,
  deleteCustomChatPrompt,
  saveCustomChatPrompt,
  useCustomChatPrompts,
} from "~/chat/custom-prompts";
import { SettingsPageTitle } from "~/settings/page-title";
import { SETTING_CONTROL_CLASS } from "~/settings/setting-row";
import { id } from "~/shared/utils";

export function SettingsChatPrompts() {
  const { t } = useLingui();
  const prompts = useCustomChatPrompts();
  const [draft, setDraft] = useState<CustomChatPrompt | null>(null);
  const remove = useMutation({ mutationFn: deleteCustomChatPrompt });
  const availabilityOptions: Array<{
    value: CustomChatPromptAvailability;
    label: string;
  }> = [
    { value: "always", label: t`Always` },
    { value: "recording", label: t`While recording` },
    { value: "not_recording", label: t`When not recording` },
  ];
  const isAdding =
    draft !== null && !prompts.some((prompt) => prompt.id === draft.id);

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-2">
        <SettingsPageTitle title={<Trans>Chat prompts</Trans>} />
        <p className="text-muted-foreground text-sm">
          <Trans>
            Save prompts you use often. Pick one from the suggestions in a new
            chat, or type / in the chat box to add it to your message.
          </Trans>
        </p>
      </div>

      {prompts.length === 0 && !isAdding ? (
        <div className="border-border bg-card flex min-h-40 flex-col items-center justify-center rounded-2xl border px-6 text-center">
          <ChatCenteredDots className="text-muted-foreground mb-3 size-5" />
          <p className="text-sm font-medium">
            <Trans>No chat prompts yet</Trans>
          </p>
          <p className="text-muted-foreground mt-1 max-w-sm text-xs">
            <Trans>
              Tip: Add a prompt you ask in every meeting, like drafting talking
              points while you record.
            </Trans>
          </p>
        </div>
      ) : (
        <div className="border-border bg-card divide-border divide-y overflow-hidden rounded-2xl border">
          {prompts.map((prompt) =>
            draft?.id === prompt.id ? (
              <ChatPromptEditor
                key={prompt.id}
                draft={draft}
                availabilityOptions={availabilityOptions}
                onDone={() => setDraft(null)}
              />
            ) : (
              <ChatPromptRow
                key={prompt.id}
                prompt={prompt}
                availabilityLabel={
                  availabilityOptions.find(
                    (option) => option.value === prompt.availability,
                  )?.label ?? ""
                }
                isDeleting={remove.isPending}
                onEdit={() => setDraft(prompt)}
                onDelete={() => remove.mutate(prompt.id)}
              />
            ),
          )}
          {isAdding && (
            <ChatPromptEditor
              key={draft.id}
              draft={draft}
              availabilityOptions={availabilityOptions}
              onDone={() => setDraft(null)}
            />
          )}
        </div>
      )}

      {remove.isError && (
        <p role="alert" className="text-destructive text-sm">
          <Trans>Couldn't delete this prompt. Try again.</Trans>
        </p>
      )}

      {draft === null && (
        <div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() =>
              setDraft({
                id: id(),
                title: "",
                prompt: "",
                availability: "always",
              })
            }
          >
            <Plus className="size-3.5" />
            <Trans>Add prompt</Trans>
          </Button>
        </div>
      )}
    </div>
  );
}

function ChatPromptRow({
  prompt,
  availabilityLabel,
  isDeleting,
  onEdit,
  onDelete,
}: {
  prompt: CustomChatPrompt;
  availabilityLabel: string;
  isDeleting: boolean;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const { t } = useLingui();
  const title = prompt.title;

  return (
    <div className="group flex min-h-12 items-center justify-between gap-3 py-3 pr-3 pl-4">
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium">{title}</p>
        <p className="text-muted-foreground truncate text-xs">
          {availabilityLabel} · {prompt.prompt}
        </p>
      </div>
      <div className="flex items-center">
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="text-muted-foreground hover:text-foreground size-7 shrink-0 opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
          onClick={onEdit}
          aria-label={t`Edit ${title}`}
        >
          <PencilSimple className="size-4" />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="text-muted-foreground hover:text-foreground size-7 shrink-0 opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
          onClick={onDelete}
          disabled={isDeleting}
          aria-label={t`Delete ${title}`}
        >
          <MinusCircle className="size-4" />
        </Button>
      </div>
    </div>
  );
}

function ChatPromptEditor({
  draft,
  availabilityOptions,
  onDone,
}: {
  draft: CustomChatPrompt;
  availabilityOptions: Array<{
    value: CustomChatPromptAvailability;
    label: string;
  }>;
  onDone: () => void;
}) {
  const { t } = useLingui();
  const availabilityLabelId = useId();
  const save = useMutation({
    mutationFn: saveCustomChatPrompt,
    onSuccess: onDone,
  });
  const form = useForm({
    defaultValues: {
      title: draft.title,
      prompt: draft.prompt,
      availability: draft.availability,
    },
    onSubmit: ({ value }) =>
      save.mutate({
        id: draft.id,
        title: value.title.trim(),
        prompt: value.prompt.trim(),
        availability: value.availability,
      }),
  });

  return (
    <form
      className="flex flex-col gap-4 p-4"
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit();
      }}
    >
      <form.Field name="title">
        {(field) => (
          <label className="flex flex-col gap-2 text-sm">
            <Trans>Name</Trans>
            <Input
              autoFocus
              value={field.state.value}
              onChange={(event) => field.handleChange(event.target.value)}
              onBlur={field.handleBlur}
              placeholder={t`Draft talking points`}
            />
          </label>
        )}
      </form.Field>
      <form.Field name="prompt">
        {(field) => (
          <label className="flex flex-col gap-2 text-sm">
            <Trans>Prompt</Trans>
            <Textarea
              rows={10}
              value={field.state.value}
              onChange={(event) => field.handleChange(event.target.value)}
              onBlur={field.handleBlur}
              placeholder={t`Tell Chat what to do with the meeting.`}
            />
          </label>
        )}
      </form.Field>
      <form.Field name="availability">
        {(field) => (
          <div className="flex flex-col gap-2 text-sm">
            <span id={availabilityLabelId}>
              <Trans>When to show</Trans>
            </span>
            <div className="w-48 max-w-full">
              <Select
                value={field.state.value}
                onValueChange={(value) =>
                  field.handleChange(value as CustomChatPromptAvailability)
                }
              >
                <SelectTrigger
                  aria-labelledby={availabilityLabelId}
                  className={SETTING_CONTROL_CLASS}
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {availabilityOptions.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
        )}
      </form.Field>
      {save.isError && (
        <p role="alert" className="text-destructive text-sm">
          <Trans>Couldn't save this prompt. Try again.</Trans>
        </p>
      )}
      <div className="flex justify-end gap-2">
        <Button type="button" variant="ghost" size="sm" onClick={onDone}>
          <Trans>Cancel</Trans>
        </Button>
        <form.Subscribe
          selector={(state) =>
            state.values.title.trim() !== "" &&
            state.values.prompt.trim() !== ""
          }
        >
          {(canSave) => (
            <Button
              type="submit"
              size="sm"
              disabled={!canSave || save.isPending}
            >
              <Trans>Save</Trans>
            </Button>
          )}
        </form.Subscribe>
      </div>
    </form>
  );
}
