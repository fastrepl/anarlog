import { useForm } from "@tanstack/react-form";
import { useEffect } from "react";

import { Input } from "@hypr/ui/components/ui/input";

import { useSetSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";

export const TODO_FILTER_SETTING_KEYS = {
  github: "todo_github_repository",
} as const;

export type TodoFilterSettingKey =
  (typeof TODO_FILTER_SETTING_KEYS)[keyof typeof TODO_FILTER_SETTING_KEYS];

export function TodoFilterField({
  settingKey,
  label,
  description,
  placeholder,
  invalidMessage,
}: {
  settingKey: TodoFilterSettingKey;
  label: string;
  description: string;
  placeholder: string;
  invalidMessage?: string;
}) {
  const storedValue = useConfigValue(settingKey) ?? "";
  const setValue = useSetSettingValue(settingKey);

  const form = useForm({
    defaultValues: { value: storedValue },
    listeners: {
      onChange: ({ formApi }) => {
        void formApi.handleSubmit();
      },
    },
    onSubmit: ({ value }) => {
      setValue(value.value);
    },
  });

  useEffect(() => {
    if (form.getFieldValue("value") === storedValue) {
      return;
    }
    form.setFieldValue("value", storedValue);
  }, [form, storedValue]);

  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex-1">
        <h3 className="mb-1 text-sm font-medium">{label}</h3>
        <p className="text-muted-foreground text-xs">{description}</p>
        {invalidMessage ? (
          <p className="mt-2 text-xs text-red-600">{invalidMessage}</p>
        ) : null}
      </div>

      <form.Field name="value">
        {(field) => (
          <Input
            className="w-52"
            placeholder={placeholder}
            value={field.state.value}
            onChange={(event) => field.handleChange(event.target.value)}
          />
        )}
      </form.Field>
    </div>
  );
}
