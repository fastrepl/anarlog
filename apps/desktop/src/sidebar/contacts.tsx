import { Trans } from "@lingui/react/macro";
import { Reorder } from "motion/react";
import { useCallback, useMemo, useRef, useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import type { ContactsSelection } from "@hypr/plugin-windows";

import { NewPersonForm } from "~/contacts/new-person-form";
import { OrganizationItem } from "~/contacts/organization-item";
import { PersonItem } from "~/contacts/person-item";
import {
  deleteHuman,
  deleteOrganization,
  type HumanRecord,
  type OrganizationRecord,
  reorderPinnedContacts,
  useHumans,
  useOrganizations,
} from "~/contacts/queries";
import { ColumnHeader, type SortOption } from "~/contacts/shared";
import { useTabs } from "~/store/zustand/tabs";

type ContactItem =
  | { kind: "person"; id: string; person: HumanRecord }
  | {
      kind: "organization";
      id: string;
      organization: OrganizationRecord;
    };

export function ContactsNav() {
  const currentTab = useTabs((state) => state.currentTab);
  const updateContactsTabState = useTabs(
    (state) => state.updateContactsTabState,
  );
  const invalidateResource = useTabs((state) => state.invalidateResource);

  const selected =
    currentTab?.type === "contacts" ? currentTab.state.selected : null;

  const setSelected = useCallback(
    (value: ContactsSelection | null) => {
      if (currentTab?.type === "contacts") {
        updateContactsTabState(currentTab, { selected: value });
      }
    },
    [currentTab, updateContactsTabState],
  );

  const handleDeletePerson = useCallback(
    (id: string) => {
      invalidateResource("humans", id);
      void deleteHuman(id).catch((error) => {
        console.error("[contacts] failed to delete contact", error);
      });
      setSelected(null);
    },
    [invalidateResource, setSelected],
  );

  const handleDeleteOrganization = useCallback(
    (id: string) => {
      invalidateResource("organizations" as const, id);
      void deleteOrganization(id).catch((error) => {
        console.error("[contacts] failed to delete organization", error);
      });
      setSelected(null);
    },
    [invalidateResource, setSelected],
  );

  return (
    <ContactsList
      selected={selected}
      setSelected={setSelected}
      onDeletePerson={handleDeletePerson}
      onDeleteOrganization={handleDeleteOrganization}
    />
  );
}

function ContactsList({
  selected,
  setSelected,
  onDeletePerson,
  onDeleteOrganization,
}: {
  selected: ContactsSelection | null;
  setSelected: (value: ContactsSelection | null) => void;
  onDeletePerson: (id: string) => void;
  onDeleteOrganization: (id: string) => void;
}) {
  const [showNewPerson, setShowNewPerson] = useState(false);
  const [searchValue, setSearchValue] = useState("");
  const [sortOption, setSortOption] = useState<SortOption>("alphabetical");
  const searchInputRef = useRef<HTMLInputElement>(null);

  useHotkeys(
    "mod+f",
    () => {
      searchInputRef.current?.focus();
      searchInputRef.current?.select();
    },
    { preventDefault: true, enableOnFormTags: true },
    [],
  );

  const humans = useHumans();
  const organizations = useOrganizations();

  const { pinnedItems, nonPinnedItems } = useMemo(() => {
    const q = searchValue.toLowerCase().trim();
    const compare = (
      a: { name: string; createdAt: string },
      b: {
        name: string;
        createdAt: string;
      },
    ) => {
      if (sortOption === "alphabetical") return a.name.localeCompare(b.name);
      if (sortOption === "reverse-alphabetical") {
        return b.name.localeCompare(a.name);
      }
      const byCreatedAt = a.createdAt.localeCompare(b.createdAt);
      return sortOption === "newest" ? -byCreatedAt : byCreatedAt;
    };
    const filteredHumans = humans
      .filter((human) => {
        if (!q) return true;
        return [human.name, human.email, human.phone].some((value) =>
          value.toLowerCase().includes(q),
        );
      })
      .sort(compare);
    const filteredOrganizations = organizations
      .filter(
        (organization) => !q || organization.name.toLowerCase().includes(q),
      )
      .sort(compare);
    const allPinned: ContactItem[] = [
      ...filteredHumans
        .filter((human) => human.pinned)
        .map((person) => ({
          kind: "person" as const,
          id: person.id,
          person,
        })),
      ...filteredOrganizations
        .filter((organization) => organization.pinned)
        .map((organization) => ({
          kind: "organization" as const,
          id: organization.id,
          organization,
        })),
    ].sort((a, b) => {
      const aOrder =
        a.kind === "person" ? a.person.pinOrder : a.organization.pinOrder;
      const bOrder =
        b.kind === "person" ? b.person.pinOrder : b.organization.pinOrder;
      return (aOrder ?? Infinity) - (bOrder ?? Infinity);
    });
    const unpinnedOrgs: ContactItem[] = filteredOrganizations
      .filter((organization) => !organization.pinned)
      .map((organization) => ({
        kind: "organization" as const,
        id: organization.id,
        organization,
      }));
    const unpinnedPeople: ContactItem[] = filteredHumans
      .filter((human) => !human.pinned)
      .map((person) => ({ kind: "person" as const, id: person.id, person }));

    return {
      pinnedItems: allPinned,
      nonPinnedItems: [...unpinnedOrgs, ...unpinnedPeople],
    };
  }, [humans, organizations, searchValue, sortOption]);

  const handleReorderPinned = useCallback(
    (newOrder: string[]) => {
      const contacts = newOrder.flatMap((id) => {
        const item = pinnedItems.find((candidate) => candidate.id === id);
        return item
          ? [
              {
                type:
                  item.kind === "person"
                    ? ("human" as const)
                    : ("organization" as const),
                id,
              },
            ]
          : [];
      });
      void reorderPinnedContacts(contacts).catch((error) => {
        console.error("[contacts] failed to reorder pins", error);
      });
    },
    [pinnedItems],
  );

  const handleAdd = useCallback(() => {
    setShowNewPerson(true);
  }, []);

  const isActive = (item: ContactItem) => {
    if (!selected) return false;
    return selected.type === item.kind && selected.id === item.id;
  };

  return (
    <div className="flex h-full w-full flex-col">
      <ColumnHeader
        title={<Trans>Contacts</Trans>}
        sortOption={sortOption}
        setSortOption={setSortOption}
        onAdd={handleAdd}
        searchValue={searchValue}
        onSearchChange={setSearchValue}
        searchInputRef={searchInputRef}
      />
      <div className="scrollbar-hide flex-1 overflow-y-auto">
        {showNewPerson && (
          <NewPersonForm
            onSave={(humanId) => {
              setShowNewPerson(false);
              setSelected({ type: "person", id: humanId });
            }}
            onCancel={() => setShowNewPerson(false)}
          />
        )}
        {pinnedItems.length > 0 && !searchValue.trim() && (
          <Reorder.Group
            axis="y"
            values={pinnedItems.map((i) => i.id)}
            onReorder={handleReorderPinned}
            className="flex flex-col"
          >
            {pinnedItems.map((item) => (
              <Reorder.Item key={item.id} value={item.id}>
                {item.kind === "person" ? (
                  <PersonItem
                    active={isActive(item)}
                    person={item.person}
                    onClick={() => setSelected({ type: "person", id: item.id })}
                    onDelete={onDeletePerson}
                  />
                ) : (
                  <OrganizationItem
                    active={isActive(item)}
                    organization={item.organization}
                    onClick={() =>
                      setSelected({ type: "organization", id: item.id })
                    }
                    onDelete={onDeleteOrganization}
                  />
                )}
              </Reorder.Item>
            ))}
          </Reorder.Group>
        )}
        {pinnedItems.length > 0 && searchValue.trim() && (
          <div className="flex flex-col">
            {pinnedItems.map((item) =>
              item.kind === "person" ? (
                <PersonItem
                  key={`pinned-person-${item.id}`}
                  active={isActive(item)}
                  person={item.person}
                  onClick={() => setSelected({ type: "person", id: item.id })}
                  onDelete={onDeletePerson}
                />
              ) : (
                <OrganizationItem
                  key={`pinned-org-${item.id}`}
                  active={isActive(item)}
                  organization={item.organization}
                  onClick={() =>
                    setSelected({ type: "organization", id: item.id })
                  }
                  onDelete={onDeleteOrganization}
                />
              ),
            )}
          </div>
        )}
        {pinnedItems.length > 0 && nonPinnedItems.length > 0 && (
          <div className="bg-accent mx-3 my-1 h-px" />
        )}
        {nonPinnedItems.map((item) =>
          item.kind === "person" ? (
            <PersonItem
              key={`person-${item.id}`}
              active={isActive(item)}
              person={item.person}
              onClick={() => setSelected({ type: "person", id: item.id })}
              onDelete={onDeletePerson}
            />
          ) : (
            <OrganizationItem
              key={`org-${item.id}`}
              active={isActive(item)}
              organization={item.organization}
              onClick={() => setSelected({ type: "organization", id: item.id })}
              onDelete={onDeleteOrganization}
            />
          ),
        )}
      </div>
    </div>
  );
}
