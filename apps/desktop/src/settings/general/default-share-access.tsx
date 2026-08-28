import { Trans, useLingui } from "@lingui/react/macro";
import type { ReactNode } from "react";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";

import { useAuth } from "~/auth";
import {
  normalizeDefaultMeetingShareAccess,
  type DefaultMeetingShareAccess,
} from "~/session-sharing/default-access";
import { useAvailableShareWorkspaces } from "~/session-sharing/source";
import { useSetSettingValue } from "~/settings/queries";
import { settingControlStyles, SettingRow } from "~/settings/setting-row";
import { useConfigValue } from "~/shared/config";

export function DefaultMeetingShareAccessSelector() {
  const { t } = useLingui();
  const value = normalizeDefaultMeetingShareAccess(
    useConfigValue("default_meeting_share_access"),
  );
  const setValue = useSetSettingValue("default_meeting_share_access");
  const auth = useAuth();
  const workspaces = useAvailableShareWorkspaces(auth.session?.user.id ?? null);
  const workspaceLabel = workspaces[0]?.name;

  const handleChange = (nextValue: string) => {
    setValue(normalizeDefaultMeetingShareAccess(nextValue));
  };

  return (
    <SettingRow
      title={<Trans>Default sharing</Trans>}
      description={
        <Trans>Choose who can access notes from new meetings.</Trans>
      }
    >
      {(labelProps) => (
        <Select value={value} onValueChange={handleChange}>
          <SelectTrigger {...labelProps} sx={settingControlStyles.control}>
            <SelectValue placeholder={t`Select default sharing`} />
          </SelectTrigger>
          <SelectContent>
            <DefaultShareAccessOption value="me">
              <Trans>Only me</Trans>
            </DefaultShareAccessOption>
            <DefaultShareAccessOption value="participants">
              <Trans>People in the meeting</Trans>
            </DefaultShareAccessOption>
            <DefaultShareAccessOption
              value="workspace"
              disabled={workspaces.length === 0}
            >
              {workspaceLabel ? (
                <Trans>Everyone in {workspaceLabel}</Trans>
              ) : (
                <Trans>Everyone in the workspace</Trans>
              )}
            </DefaultShareAccessOption>
          </SelectContent>
        </Select>
      )}
    </SettingRow>
  );
}

function DefaultShareAccessOption({
  value,
  disabled,
  children,
}: {
  value: DefaultMeetingShareAccess;
  disabled?: boolean;
  children: ReactNode;
}) {
  return (
    <SelectItem value={value} disabled={disabled}>
      {children}
    </SelectItem>
  );
}
