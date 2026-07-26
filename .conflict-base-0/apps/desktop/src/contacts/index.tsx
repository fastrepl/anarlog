import { useCallback } from "react";

import type { ContactsSelection } from "@hypr/plugin-windows";

import { DetailsColumn } from "./details";
import { OrganizationDetailsColumn } from "./organization-details";
import { useHumans, useOrganizations } from "./queries";

import { StandardContentWrapper } from "~/shared/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export function TabContentContact({
  tab,
}: {
  tab: Extract<Tab, { type: "contacts" }>;
}) {
  return (
    <StandardContentWrapper>
      <ContactView tab={tab} />
    </StandardContentWrapper>
  );
}

function ContactView({ tab }: { tab: Extract<Tab, { type: "contacts" }> }) {
  const updateContactsTabState = useTabs(
    (state) => state.updateContactsTabState,
  );
  const openCurrent = useTabs((state) => state.openCurrent);

  const selected = tab.state.selected;
  const humans = useHumans();
  const organizations = useOrganizations();

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

  const effectiveSelection =
    selected ??
    (humans[0]
      ? ({ type: "person", id: humans[0].id } as const)
      : organizations[0]
        ? ({ type: "organization", id: organizations[0].id } as const)
        : null);

  return (
    <div className="h-full">
      {effectiveSelection?.type === "organization" ? (
        <OrganizationDetailsColumn
          organization={
            organizations.find(
              (organization) => organization.id === effectiveSelection.id,
            ) ?? null
          }
          humans={humans}
          onPersonClick={(personId) =>
            setSelected({ type: "person", id: personId })
          }
        />
      ) : (
        <DetailsColumn
          human={
            effectiveSelection?.type === "person"
              ? (humans.find((human) => human.id === effectiveSelection.id) ??
                null)
              : null
          }
          humans={humans}
          organizations={organizations}
          handleSessionClick={handleSessionClick}
        />
      )}
    </div>
  );
}
