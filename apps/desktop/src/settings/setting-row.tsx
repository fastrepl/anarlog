import * as stylex from "@stylexjs/stylex";
import { type ReactNode, useId } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import { Switch } from "@anlg/ui/components/ui/switch";

export const settingControlStyles = stylex.create({
  control: {
    backgroundColor: colors.card,
    boxShadow: {
      default: "none",
      ":focus": "none",
    },
    height: "2.25rem",
    width: "100%",
  },
});

export function SettingRow({
  title,
  description,
  controlWidth = "fixed",
  children,
}: {
  title: ReactNode;
  description?: ReactNode;
  controlWidth?: "fixed" | "content";
  children: (labelProps: {
    "aria-labelledby": string;
    "aria-describedby": string | undefined;
  }) => ReactNode;
}) {
  const titleId = useId();
  const descriptionId = useId();

  return (
    <div {...stylex.props(styles.row)}>
      <div {...stylex.props(styles.copy)}>
        <h3 id={titleId} {...stylex.props(styles.title)}>
          {title}
        </h3>
        {description && (
          <p id={descriptionId} {...stylex.props(styles.description)}>
            {description}
          </p>
        )}
      </div>
      <div
        {...stylex.props(
          styles.control,
          controlWidth === "fixed" && styles.fixedControl,
        )}
      >
        {children({
          "aria-labelledby": titleId,
          "aria-describedby": description ? descriptionId : undefined,
        })}
      </div>
    </div>
  );
}

export function SettingSwitchRow({
  title,
  description,
  checked,
  onChange,
  disabled = false,
}: {
  title: ReactNode;
  description?: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <SettingRow title={title} description={description} controlWidth="content">
      {(labelProps) => (
        <Switch
          {...labelProps}
          checked={checked}
          onCheckedChange={onChange}
          disabled={disabled}
        />
      )}
    </SettingRow>
  );
}

const styles = stylex.create({
  control: {
    display: "flex",
    flexShrink: 0,
    justifyContent: "flex-end",
  },
  copy: {
    flex: "1",
    minWidth: 0,
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  fixedControl: {
    width: "12rem",
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

export { styles as settingRowStyles };
