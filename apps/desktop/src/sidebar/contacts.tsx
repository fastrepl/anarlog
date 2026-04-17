import { Reorder } from "motion/react";
import { useCallback, useMemo, useRef, useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import type { ContactsSelection } from "@hypr/plugin-windows";

import {
  type PinnedContactItem,
  useAllHumans,
  useAllOrganizations,
  useDeleteHuman,
  useDeleteOrganization,
  useReorderPinnedContacts,
  useSortedVisibleHumanIds,
  useSortedVisibleOrganizationIds,
} from "~/contacts/hooks";
import { NewPersonForm } from "~/contacts/new-person-form";
import { OrganizationItem } from "~/contacts/organization-item";
import { PersonItem } from "~/contacts/person-item";
import { ColumnHeader, type SortOption } from "~/contacts/shared";
import { useTabs } from "~/store/zustand/tabs";

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

  const deletePerson = useDeleteHuman();

  const handleDeletePerson = useCallback(
    (id: string) => {
      invalidateResource("humans", id);
      deletePerson(id);
      setSelected(null);
    },
    [invalidateResource, deletePerson, setSelected],
  );

  const deleteOrganization = useDeleteOrganization();

  const handleDeleteOrganization = useCallback(
    (id: string) => {
      invalidateResource("organizations" as const, id);
      deleteOrganization(id);
      setSelected(null);
    },
    [invalidateResource, deleteOrganization, setSelected],
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

  const allHumans = useAllHumans();
  const allOrgs = useAllOrganizations();
  const reorderPinned = useReorderPinnedContacts();

  const alphabeticalHumanIds = useSortedVisibleHumanIds("name", false);
  const reverseAlphabeticalHumanIds = useSortedVisibleHumanIds("name", true);
  const newestHumanIds = useSortedVisibleHumanIds("created_at", true);
  const oldestHumanIds = useSortedVisibleHumanIds("created_at", false);

  const alphabeticalOrgIds = useSortedVisibleOrganizationIds("name", false);
  const reverseAlphabeticalOrgIds = useSortedVisibleOrganizationIds(
    "name",
    true,
  );
  const newestOrgIds = useSortedVisibleOrganizationIds("created_at", true);
  const oldestOrgIds = useSortedVisibleOrganizationIds("created_at", false);

  const sortedHumanIds =
    sortOption === "alphabetical"
      ? alphabeticalHumanIds
      : sortOption === "reverse-alphabetical"
        ? reverseAlphabeticalHumanIds
        : sortOption === "newest"
          ? newestHumanIds
          : oldestHumanIds;

  const sortedOrgIds =
    sortOption === "alphabetical"
      ? alphabeticalOrgIds
      : sortOption === "reverse-alphabetical"
        ? reverseAlphabeticalOrgIds
        : sortOption === "newest"
          ? newestOrgIds
          : oldestOrgIds;

  const { pinnedHumanIds, unpinnedHumanIds } = useMemo(() => {
    const pinned = sortedHumanIds.filter((id) => allHumans[id]?.pinned);
    const unpinned = sortedHumanIds.filter((id) => !allHumans[id]?.pinned);

    const sortedPinned = [...pinned].sort((a, b) => {
      const orderA = allHumans[a]?.pin_order ?? Infinity;
      const orderB = allHumans[b]?.pin_order ?? Infinity;
      return orderA - orderB;
    });

    return { pinnedHumanIds: sortedPinned, unpinnedHumanIds: unpinned };
  }, [sortedHumanIds, allHumans]);

  const { pinnedOrgIds, unpinnedOrgIds } = useMemo(() => {
    const pinned = sortedOrgIds.filter((id) => allOrgs[id]?.pinned);
    const unpinned = sortedOrgIds.filter((id) => !allOrgs[id]?.pinned);

    const sortedPinned = [...pinned].sort((a, b) => {
      const orderA = allOrgs[a]?.pin_order ?? Infinity;
      const orderB = allOrgs[b]?.pin_order ?? Infinity;
      return orderA - orderB;
    });

    return { pinnedOrgIds: sortedPinned, unpinnedOrgIds: unpinned };
  }, [sortedOrgIds, allOrgs]);

  const { pinnedItems, nonPinnedItems } = useMemo(() => {
    const q = searchValue.toLowerCase().trim();

    const filterHuman = (id: string) => {
      if (!q) return true;
      const human = allHumans[id];
      const name = (human?.name ?? "").toLowerCase();
      const email = (human?.email ?? "").toLowerCase();
      return name.includes(q) || email.includes(q);
    };

    const filterOrg = (id: string) => {
      if (!q) return true;
      const name = (allOrgs[id]?.name ?? "").toLowerCase();
      return name.includes(q);
    };

    const allPinned = [
      ...pinnedHumanIds.filter(filterHuman).map((id) => ({
        kind: "person" as const,
        id,
        pin_order: allHumans[id]?.pin_order ?? Infinity,
      })),
      ...pinnedOrgIds.filter(filterOrg).map((id) => ({
        kind: "organization" as const,
        id,
        pin_order: allOrgs[id]?.pin_order ?? Infinity,
      })),
    ]
      .sort((a, b) => a.pin_order - b.pin_order)
      .map(({ kind, id }) => ({ kind, id }));

    const unpinnedOrgs: PinnedContactItem[] = unpinnedOrgIds
      .filter(filterOrg)
      .map((id) => ({ kind: "organization" as const, id }));

    const unpinnedPeople: PinnedContactItem[] = unpinnedHumanIds
      .filter(filterHuman)
      .map((id) => ({ kind: "person" as const, id }));

    return {
      pinnedItems: allPinned,
      nonPinnedItems: [...unpinnedOrgs, ...unpinnedPeople],
    };
  }, [
    pinnedHumanIds,
    unpinnedHumanIds,
    pinnedOrgIds,
    unpinnedOrgIds,
    allOrgs,
    allHumans,
    searchValue,
  ]);

  const handleReorderPinned = useCallback(
    (newOrder: string[]) => {
      const ordered: PinnedContactItem[] = [];
      for (const id of newOrder) {
        const item = pinnedItems.find((i) => i.id === id);
        if (item) ordered.push(item);
      }
      reorderPinned(ordered);
    },
    [reorderPinned, pinnedItems],
  );

  const handleAdd = useCallback(() => {
    setShowNewPerson(true);
  }, []);

  const isActive = (item: PinnedContactItem) => {
    if (!selected) return false;
    return selected.type === item.kind && selected.id === item.id;
  };

  return (
    <div className="flex h-full w-full flex-col">
      <ColumnHeader
        title="Contacts"
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
                    humanId={item.id}
                    onClick={() => setSelected({ type: "person", id: item.id })}
                    onDelete={onDeletePerson}
                  />
                ) : (
                  <OrganizationItem
                    active={isActive(item)}
                    organizationId={item.id}
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
                  humanId={item.id}
                  onClick={() => setSelected({ type: "person", id: item.id })}
                  onDelete={onDeletePerson}
                />
              ) : (
                <OrganizationItem
                  key={`pinned-org-${item.id}`}
                  active={isActive(item)}
                  organizationId={item.id}
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
          <div className="mx-3 my-1 h-px bg-neutral-200" />
        )}
        {nonPinnedItems.map((item) =>
          item.kind === "person" ? (
            <PersonItem
              key={`person-${item.id}`}
              active={isActive(item)}
              humanId={item.id}
              onClick={() => setSelected({ type: "person", id: item.id })}
              onDelete={onDeletePerson}
            />
          ) : (
            <OrganizationItem
              key={`org-${item.id}`}
              active={isActive(item)}
              organizationId={item.id}
              onClick={() => setSelected({ type: "organization", id: item.id })}
              onDelete={onDeleteOrganization}
            />
          ),
        )}
      </div>
    </div>
  );
}
