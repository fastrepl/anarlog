import { Trans } from "@lingui/react/macro";

import { StandardTabWrapper } from "~/shared/main";
import { type Tab } from "~/store/zustand/tabs";

export function TabContentHuman({
  tab: _,
}: {
  tab: Extract<Tab, { type: "humans" }>;
}) {
  return (
    <StandardTabWrapper>
      <div>
        <Trans>Human</Trans>
      </div>
    </StandardTabWrapper>
  );
}
