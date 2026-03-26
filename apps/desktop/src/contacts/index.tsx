import { Contact2Icon } from "lucide-react";
import { useCallback, useEffect } from "react";

import type { ContactsSelection } from "@hypr/plugin-windows";

import { DetailsColumn } from "./details";
import { OrganizationDetailsColumn } from "./organization-details";

import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import * as main from "~/store/tinybase/store/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export const TabItemContact: TabItem<Extract<Tab, { type: "contacts" }>> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}) => {
  return (
    <TabItemBase
      icon={<Contact2Icon className="h-4 w-4" />}
      title={"Contacts"}
      selected={tab.active}
      pinned={tab.pinned}
      tabIndex={tabIndex}
      handleCloseThis={() => handleCloseThis(tab)}
      handleSelectThis={() => handleSelectThis(tab)}
      handleCloseOthers={handleCloseOthers}
      handleCloseAll={handleCloseAll}
      handlePinThis={() => handlePinThis(tab)}
      handleUnpinThis={() => handleUnpinThis(tab)}
    />
  );
};

export function TabContentContact({
  tab,
}: {
  tab: Extract<Tab, { type: "contacts" }>;
}) {
  return (
    <StandardTabWrapper>
      <ContactView tab={tab} />
    </StandardTabWrapper>
  );
}

function ContactView({ tab }: { tab: Extract<Tab, { type: "contacts" }> }) {
  const updateContactsTabState = useTabs(
    (state) => state.updateContactsTabState,
  );
  const openCurrent = useTabs((state) => state.openCurrent);

  const selected = tab.state.selected;

  const setSelected = useCallback(
    (value: ContactsSelection | null) => {
      updateContactsTabState(tab, { selected: value });
    },
    [updateContactsTabState, tab],
  );

  const handleSessionClick = useCallback(
    (id: string) => {
      openCurrent({ type: "sessions", id });
    },
    [openCurrent],
  );

  const allHumanIds = main.UI.useResultSortedRowIds(
    main.QUERIES.visibleHumans,
    "name",
    false,
    0,
    undefined,
    main.STORE_ID,
  );

  const allOrgIds = main.UI.useResultSortedRowIds(
    main.QUERIES.visibleOrganizations,
    "name",
    false,
    0,
    undefined,
    main.STORE_ID,
  );

  useEffect(() => {
    if (!selected) {
      if (allHumanIds.length > 0) {
        setSelected({ type: "person", id: allHumanIds[0] });
      } else if (allOrgIds.length > 0) {
        setSelected({ type: "organization", id: allOrgIds[0] });
      }
    }
  }, [allHumanIds, allOrgIds, selected, setSelected]);

  return (
    <div className="h-full">
      {selected?.type === "organization" ? (
        <OrganizationDetailsColumn
          selectedOrganizationId={selected.id}
          onPersonClick={(personId) =>
            setSelected({ type: "person", id: personId })
          }
        />
      ) : (
        <DetailsColumn
          selectedHumanId={selected?.type === "person" ? selected.id : null}
          handleSessionClick={handleSessionClick}
        />
      )}
    </div>
  );
}
