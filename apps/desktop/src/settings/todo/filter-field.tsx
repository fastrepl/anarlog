import * as stylex from "@stylexjs/stylex";
import { useForm } from "@tanstack/react-form";

import { colors } from "@anlg/design-system/tokens.stylex";
import { Input } from "@anlg/ui/components/ui/input";

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

  return (
    <div {...stylex.props(styles.row)}>
      <div {...stylex.props(styles.copy)}>
        <h3 {...stylex.props(styles.title)}>{label}</h3>
        <p {...stylex.props(styles.description)}>{description}</p>
        {invalidMessage ? (
          <p {...stylex.props(styles.error)}>{invalidMessage}</p>
        ) : null}
      </div>

      <form.Field name="value">
        {(field) => (
          <Input
            sx={styles.input}
            placeholder={placeholder}
            value={field.state.value}
            onChange={(event) => field.handleChange(event.target.value)}
          />
        )}
      </form.Field>
    </div>
  );
}

const styles = stylex.create({
  copy: {
    flex: "1",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  error: {
    color: "rgb(220 38 38)",
    fontSize: "0.75rem",
    lineHeight: "1rem",
    marginTop: "0.5rem",
  },
  input: {
    width: "13rem",
  },
  row: {
    alignItems: "center",
    display: "flex",
    gap: "1rem",
    justifyContent: "space-between",
  },
  title: {
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
    marginBottom: "0.25rem",
  },
});
