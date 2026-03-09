import { Facehash } from "facehash";
import { Building2, CornerDownLeft, Pin } from "lucide-react";
import { Reorder } from "motion/react";
import React, { useCallback, useMemo, useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import type { ContactsSelection } from "@hypr/plugin-windows";
import { cn } from "@hypr/utils";

import { ColumnHeader, getContactBgClass, type SortOption } from "./shared";

import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";
import * as main from "~/store/tinybase/store/main";

type ContactItem =
  | { kind: "person"; id: string }
  | { kind: "organization"; id: string };

export function ContactsListColumn({
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
  const [showSearch, setShowSearch] = useState(false);

  useHotkeys(
    "mod+f",
    () => setShowSearch(true),
    { preventDefault: true, enableOnFormTags: true },
    [setShowSearch],
  );

  const allHumans = main.UI.useTable("humans", main.STORE_ID);
  const allOrgs = main.UI.useTable("organizations", main.STORE_ID);
  const store = main.UI.useStore(main.STORE_ID);

  const alphabeticalHumanIds = main.UI.useResultSortedRowIds(
    main.QUERIES.visibleHumans,
    "name",
    false,
    0,
    undefined,
    main.STORE_ID,
  );
  const reverseAlphabeticalHumanIds = main.UI.useResultSortedRowIds(
    main.QUERIES.visibleHumans,
    "name",
    true,
    0,
    undefined,
    main.STORE_ID,
  );
  const newestHumanIds = main.UI.useResultSortedRowIds(
    main.QUERIES.visibleHumans,
    "created_at",
    true,
    0,
    undefined,
    main.STORE_ID,
  );
  const oldestHumanIds = main.UI.useResultSortedRowIds(
    main.QUERIES.visibleHumans,
    "created_at",
    false,
    0,
    undefined,
    main.STORE_ID,
  );

  const alphabeticalOrgIds = main.UI.useResultSortedRowIds(
    main.QUERIES.visibleOrganizations,
    "name",
    false,
    0,
    undefined,
    main.STORE_ID,
  );
  const reverseAlphabeticalOrgIds = main.UI.useResultSortedRowIds(
    main.QUERIES.visibleOrganizations,
    "name",
    true,
    0,
    undefined,
    main.STORE_ID,
  );
  const newestOrgIds = main.UI.useResultSortedRowIds(
    main.QUERIES.visibleOrganizations,
    "created_at",
    true,
    0,
    undefined,
    main.STORE_ID,
  );
  const oldestOrgIds = main.UI.useResultSortedRowIds(
    main.QUERIES.visibleOrganizations,
    "created_at",
    false,
    0,
    undefined,
    main.STORE_ID,
  );

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
      const orderA =
        (allHumans[a]?.pin_order as number | undefined) ?? Infinity;
      const orderB =
        (allHumans[b]?.pin_order as number | undefined) ?? Infinity;
      return orderA - orderB;
    });

    return { pinnedHumanIds: sortedPinned, unpinnedHumanIds: unpinned };
  }, [sortedHumanIds, allHumans]);

  const { pinnedOrgIds, unpinnedOrgIds } = useMemo(() => {
    const pinned = sortedOrgIds.filter((id) => allOrgs[id]?.pinned);
    const unpinned = sortedOrgIds.filter((id) => !allOrgs[id]?.pinned);

    const sortedPinned = [...pinned].sort((a, b) => {
      const orderA = (allOrgs[a]?.pin_order as number | undefined) ?? Infinity;
      const orderB = (allOrgs[b]?.pin_order as number | undefined) ?? Infinity;
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
        pin_order: (allHumans[id]?.pin_order as number | undefined) ?? Infinity,
      })),
      ...pinnedOrgIds.filter(filterOrg).map((id) => ({
        kind: "organization" as const,
        id,
        pin_order: (allOrgs[id]?.pin_order as number | undefined) ?? Infinity,
      })),
    ]
      .sort((a, b) => a.pin_order - b.pin_order)
      .map(({ kind, id }) => ({ kind, id }));

    const unpinnedOrgs: ContactItem[] = unpinnedOrgIds
      .filter(filterOrg)
      .map((id) => ({ kind: "organization" as const, id }));

    const unpinnedPeople: ContactItem[] = unpinnedHumanIds
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
      if (!store) return;
      store.transaction(() => {
        newOrder.forEach((id, index) => {
          const item = pinnedItems.find((i) => i.id === id);
          if (item?.kind === "person") {
            store.setCell("humans", id, "pin_order", index);
          } else if (item?.kind === "organization") {
            store.setCell("organizations", id, "pin_order", index);
          }
        });
      });
    },
    [store, pinnedItems],
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
        title="Contacts"
        sortOption={sortOption}
        setSortOption={setSortOption}
        onAdd={handleAdd}
        searchValue={searchValue}
        onSearchChange={setSearchValue}
        showSearch={showSearch}
        onShowSearchChange={setShowSearch}
      />
      <div className="flex-1 overflow-y-auto">
        <div className="p-2">
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
                      onClick={() =>
                        setSelected({ type: "person", id: item.id })
                      }
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
                onClick={() =>
                  setSelected({ type: "organization", id: item.id })
                }
                onDelete={onDeleteOrganization}
              />
            ),
          )}
        </div>
      </div>
    </div>
  );
}

function PersonItem({
  humanId,
  active,
  onClick,
  onDelete,
}: {
  humanId: string;
  active: boolean;
  onClick: () => void;
  onDelete?: (id: string) => void;
}) {
  const person = main.UI.useRow("humans", humanId, main.STORE_ID);
  const isPinned = Boolean(person.pinned);
  const personName = String(person.name ?? "");
  const personEmail = String(person.email ?? "");
  const facehashName = personName || personEmail || humanId;
  const bgClass = getContactBgClass(facehashName);

  const store = main.UI.useStore(main.STORE_ID);

  const showContextMenu = useNativeContextMenu([
    {
      id: "delete-person",
      text: "Delete Contact",
      action: () => onDelete?.(humanId),
    },
  ]);

  const handleTogglePin = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!store) return;

      const currentPinned = store.getCell("humans", humanId, "pinned");
      if (currentPinned) {
        store.setPartialRow("humans", humanId, {
          pinned: false,
          pin_order: 0,
        });
      } else {
        const allHumans = store.getTable("humans");
        const allOrgs = store.getTable("organizations");
        const maxHumanOrder = Object.values(allHumans).reduce((max, h) => {
          const order = (h.pin_order as number | undefined) ?? 0;
          return Math.max(max, order);
        }, 0);
        const maxOrgOrder = Object.values(allOrgs).reduce((max, o) => {
          const order = (o.pin_order as number | undefined) ?? 0;
          return Math.max(max, order);
        }, 0);
        store.setPartialRow("humans", humanId, {
          pinned: true,
          pin_order: Math.max(maxHumanOrder, maxOrgOrder) + 1,
        });
      }
    },
    [store, humanId],
  );

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      onContextMenu={showContextMenu}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
      className={cn([
        "group flex w-full items-center gap-2 overflow-hidden rounded-md border bg-white px-3 py-2 text-left text-sm transition-colors hover:bg-neutral-100",
        active ? "border-neutral-500 bg-neutral-100" : "border-transparent",
      ])}
    >
      <div className={cn(["shrink-0 rounded-full", bgClass])}>
        <Facehash
          name={facehashName}
          size={32}
          interactive={true}
          showInitial={true}
          colorClasses={[bgClass]}
        />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1 truncate font-medium">
          {personName || personEmail || "Unnamed"}
        </div>
        {personEmail && personName && (
          <div className="truncate text-xs text-neutral-500">{personEmail}</div>
        )}
      </div>
      <button
        onClick={handleTogglePin}
        className={cn([
          "shrink-0 rounded-xs p-1 transition-colors",
          isPinned
            ? "text-blue-600 hover:text-blue-700"
            : "text-neutral-300 opacity-0 group-hover:opacity-100 hover:text-neutral-500",
        ])}
        aria-label={isPinned ? "Unpin contact" : "Pin contact"}
      >
        <Pin className="size-3.5" fill={isPinned ? "currentColor" : "none"} />
      </button>
    </div>
  );
}

function OrganizationItem({
  organizationId,
  active,
  onClick,
  onDelete,
}: {
  organizationId: string;
  active: boolean;
  onClick: () => void;
  onDelete?: (id: string) => void;
}) {
  const organization = main.UI.useRow(
    "organizations",
    organizationId,
    main.STORE_ID,
  );
  const isPinned = Boolean(organization.pinned);
  const store = main.UI.useStore(main.STORE_ID);

  const showContextMenu = useNativeContextMenu([
    {
      id: "delete-org",
      text: "Delete Organization",
      action: () => onDelete?.(organizationId),
    },
  ]);

  const handleTogglePin = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!store) return;

      const currentPinned = store.getCell(
        "organizations",
        organizationId,
        "pinned",
      );
      if (currentPinned) {
        store.setPartialRow("organizations", organizationId, {
          pinned: false,
          pin_order: 0,
        });
      } else {
        const allOrgs = store.getTable("organizations");
        const allHumans = store.getTable("humans");
        const maxOrgOrder = Object.values(allOrgs).reduce((max, o) => {
          const order = (o.pin_order as number | undefined) ?? 0;
          return Math.max(max, order);
        }, 0);
        const maxHumanOrder = Object.values(allHumans).reduce((max, h) => {
          const order = (h.pin_order as number | undefined) ?? 0;
          return Math.max(max, order);
        }, 0);
        store.setPartialRow("organizations", organizationId, {
          pinned: true,
          pin_order: Math.max(maxOrgOrder, maxHumanOrder) + 1,
        });
      }
    },
    [store, organizationId],
  );

  if (!organization) {
    return null;
  }

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      onContextMenu={showContextMenu}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
      className={cn([
        "group flex w-full items-center gap-2 overflow-hidden rounded-md border px-3 py-2 text-left text-sm transition-colors hover:bg-neutral-100",
        active ? "border-neutral-500 bg-neutral-100" : "border-transparent",
      ])}
    >
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-neutral-100">
        <Building2 className="h-4 w-4 text-neutral-500" />
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate font-medium">{organization.name}</div>
      </div>
      <button
        onClick={handleTogglePin}
        className={cn([
          "shrink-0 rounded-xs p-1 transition-colors",
          isPinned
            ? "text-blue-600 hover:text-blue-700"
            : "text-neutral-300 opacity-0 group-hover:opacity-100 hover:text-neutral-500",
        ])}
        aria-label={isPinned ? "Unpin organization" : "Pin organization"}
      >
        <Pin className="size-3.5" fill={isPinned ? "currentColor" : "none"} />
      </button>
    </div>
  );
}

function NewPersonForm({
  onSave,
  onCancel,
}: {
  onSave: (humanId: string) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState("");
  const userId = main.UI.useValue("user_id", main.STORE_ID);

  const createHuman = main.UI.useSetRowCallback(
    "humans",
    (p: { name: string; humanId: string }) => p.humanId,
    (p: { name: string; humanId: string }) => ({
      user_id: userId || "",
      created_at: new Date().toISOString(),
      name: p.name,
      email: "",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
      pinned: false,
    }),
    [userId],
    main.STORE_ID,
  );

  const handleAdd = () => {
    const humanId = crypto.randomUUID();
    createHuman({ humanId, name: name.trim() });
    setName("");
    onSave(humanId);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (name.trim()) {
      handleAdd();
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      if (name.trim()) {
        handleAdd();
      }
    }
    if (e.key === "Escape") {
      onCancel();
    }
  };

  return (
    <div className="p-2">
      <form onSubmit={handleSubmit}>
        <div className="flex w-full items-center gap-2 rounded-xs border border-neutral-200 bg-neutral-50 px-2 py-1.5">
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Add person"
            className="w-full bg-transparent text-sm placeholder:text-neutral-400 focus:outline-hidden"
            autoFocus
          />
          {name.trim() && (
            <button
              type="submit"
              className="shrink-0 text-neutral-500 transition-colors hover:text-neutral-700"
              aria-label="Add person"
            >
              <CornerDownLeft className="size-4" />
            </button>
          )}
        </div>
      </form>
    </div>
  );
}
