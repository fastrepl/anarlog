import { t } from "@lingui/core/macro";
import type { ReactNode } from "react";

import { CircleNotch } from "@anlg/ui/components/icons";

import { useStoredSettingValuesQuery } from "~/settings/queries";

export function SettingsHydrationBoundary({
  children,
}: {
  children: ReactNode;
}) {
  const { data, isLoading, error } = useStoredSettingValuesQuery();

  if (error) {
    throw error;
  }
  if (isLoading || !data) {
    return (
      <div className="flex h-full min-h-0 min-w-0 flex-1 items-center justify-center">
        <CircleNotch
          aria-label={t`Loading settings`}
          className="text-muted-foreground size-5 animate-spin"
        />
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
      {children}
    </div>
  );
}
