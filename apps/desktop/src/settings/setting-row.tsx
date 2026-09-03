import { type ReactNode, useId } from "react";

import { Switch } from "@anlg/ui/components/ui/switch";
import { cn } from "@anlg/utils";

export const SETTING_CONTROL_CLASS =
  "bg-card h-9 w-full rounded-pill shadow-none [corner-shape:round] focus:ring-0";

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
    <div className="flex w-full min-w-0 items-center justify-between gap-4">
      <div className="min-w-0 flex-1">
        <h3 id={titleId} className="mb-1 text-sm font-medium">
          {title}
        </h3>
        {description && (
          <p id={descriptionId} className="text-muted-foreground text-xs">
            {description}
          </p>
        )}
      </div>
      <div
        className={cn([
          "flex justify-end",
          controlWidth === "fixed" && "w-48 max-w-full min-w-0",
          controlWidth !== "fixed" && "shrink-0",
        ])}
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
