import { Trans } from "@lingui/react/macro";

import { StandardContentWrapper } from "~/shared/main";
import { type Tab } from "~/store/zustand/tabs";

export function TabContentHuman({
  tab: _,
}: {
  tab: Extract<Tab, { type: "humans" }>;
}) {
  return (
    <StandardContentWrapper>
      <div>
        <Trans>Human</Trans>
      </div>
    </StandardContentWrapper>
  );
}
