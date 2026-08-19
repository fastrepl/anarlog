import { t } from "@lingui/core/macro";
import { CircleNotch } from "@phosphor-icons/react";
import type { ReactNode } from "react";

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
      <div className="flex min-h-48 items-center justify-center">
        <CircleNotch
          aria-label={t`Loading settings`}
          className="text-muted-foreground size-5 animate-spin"
        />
      </div>
    );
  }

  return children;
}
