import { useCallback, useMemo } from "react";

import * as main from "~/store/tinybase/store/main";

// Storage boundary for contacts-domain consumers. Expose normalized DTOs and
// callback-style mutations so consumer components stay storage-agnostic.
export type Human = {
  id: string;
  name: string;
  email: string;
  org_id: string;
  job_title: string;
  linkedin_username: string;
  memo: string;
  pinned: boolean;
  pin_order: number;
  created_at: string;
  user_id: string;
};

export type Organization = {
  id: string;
  name: string;
  pinned: boolean;
  pin_order: number;
  created_at: string;
  user_id: string;
};

type HumanStringField = Exclude<keyof Human, "id" | "pinned" | "pin_order">;
type OrganizationStringField = Exclude<
  keyof Organization,
  "id" | "pinned" | "pin_order"
>;

export type PersonSession = {
  id: string;
  title: string;
  created_at: string;
};

export type PersonDuplicate = {
  id: string;
  name: string;
  email: string;
};

export type SortDirection =
  | "alphabetical"
  | "reverse-alphabetical"
  | "newest"
  | "oldest";
export type PinnedContactItem =
  | { kind: "person"; id: string }
  | { kind: "organization"; id: string };

function readHuman(
  row: Record<string, unknown> | undefined,
  id: string,
): Human | null {
  if (!row || Object.keys(row).length === 0) return null;
  return {
    id,
    name: String(row.name ?? ""),
    email: String(row.email ?? ""),
    org_id: String(row.org_id ?? ""),
    job_title: String(row.job_title ?? ""),
    linkedin_username: String(row.linkedin_username ?? ""),
    memo: String(row.memo ?? ""),
    pinned: Boolean(row.pinned),
    pin_order: Number(row.pin_order ?? 0),
    created_at: String(row.created_at ?? ""),
    user_id: String(row.user_id ?? ""),
  };
}

function readOrganization(
  row: Record<string, unknown> | undefined,
  id: string,
): Organization | null {
  if (!row || Object.keys(row).length === 0) return null;
  return {
    id,
    name: String(row.name ?? ""),
    pinned: Boolean(row.pinned),
    pin_order: Number(row.pin_order ?? 0),
    created_at: String(row.created_at ?? ""),
    user_id: String(row.user_id ?? ""),
  };
}

// --- reactive reads -------------------------------------------------------

export function useHuman(id: string | null | undefined): Human | null {
  const row = main.UI.useRow("humans", id ?? "", main.STORE_ID);
  return useMemo(() => (id ? readHuman(row, id) : null), [id, row]);
}

export function useOrganization(
  id: string | null | undefined,
): Organization | null {
  const row = main.UI.useRow("organizations", id ?? "", main.STORE_ID);
  return useMemo(() => (id ? readOrganization(row, id) : null), [id, row]);
}

export function useHumanCell<K extends HumanStringField>(
  id: string,
  field: K,
): string {
  const v = main.UI.useCell("humans", id, field, main.STORE_ID);
  return (v as string | undefined) ?? "";
}

export function useOrganizationCell<K extends OrganizationStringField>(
  id: string,
  field: K,
): string {
  const v = main.UI.useCell("organizations", id, field, main.STORE_ID);
  return (v as string | undefined) ?? "";
}

export function useAllHumans(): Record<string, Human> {
  const table = main.UI.useTable("humans", main.STORE_ID);
  return useMemo(() => {
    const out: Record<string, Human> = {};
    for (const [id, row] of Object.entries(table)) {
      const h = readHuman(row as Record<string, unknown>, id);
      if (h) out[id] = h;
    }
    return out;
  }, [table]);
}

export function useHumansByIds(ids: string[]): Record<string, Human> {
  const table = main.UI.useTable("humans", main.STORE_ID);
  return useMemo(() => {
    const out: Record<string, Human> = {};
    for (const id of ids) {
      const row = table[id];
      const human = readHuman(row as Record<string, unknown> | undefined, id);
      if (human) out[id] = human;
    }
    return out;
  }, [ids, table]);
}

export function useAllOrganizations(): Record<string, Organization> {
  const table = main.UI.useTable("organizations", main.STORE_ID);
  return useMemo(() => {
    const out: Record<string, Organization> = {};
    for (const [id, row] of Object.entries(table)) {
      const o = readOrganization(row as Record<string, unknown>, id);
      if (o) out[id] = o;
    }
    return out;
  }, [table]);
}

export function useOrganizationsByIds(
  ids: string[],
): Record<string, Organization> {
  const table = main.UI.useTable("organizations", main.STORE_ID);
  return useMemo(() => {
    const out: Record<string, Organization> = {};
    for (const id of ids) {
      const row = table[id];
      const organization = readOrganization(
        row as Record<string, unknown> | undefined,
        id,
      );
      if (organization) out[id] = organization;
    }
    return out;
  }, [ids, table]);
}

export function useVisibleOrganizationList(): Organization[] {
  const table = main.UI.useResultTable(
    main.QUERIES.visibleOrganizations,
    main.STORE_ID,
  );
  return useMemo(() => {
    const out: Organization[] = [];
    for (const [id, row] of Object.entries(table)) {
      const o = readOrganization(row as Record<string, unknown>, id);
      if (o) out.push(o);
    }
    return out;
  }, [table]);
}

export function useSortedVisibleHumanIds(
  sortBy: "name" | "created_at",
  descending: boolean,
): string[] {
  return main.UI.useResultSortedRowIds(
    main.QUERIES.visibleHumans,
    sortBy,
    descending,
    0,
    undefined,
    main.STORE_ID,
  );
}

export function useSortedVisibleOrganizationIds(
  sortBy: "name" | "created_at",
  descending: boolean,
): string[] {
  return main.UI.useResultSortedRowIds(
    main.QUERIES.visibleOrganizations,
    sortBy,
    descending,
    0,
    undefined,
    main.STORE_ID,
  );
}

export function useHumansByOrg(orgId: string | null | undefined): string[] {
  return main.UI.useSliceRowIds(
    main.INDEXES.humansByOrg,
    orgId ?? "",
    main.STORE_ID,
  );
}

export function useOrganizationMembers(
  orgId: string | null | undefined,
): Human[] {
  const humanIds = useHumansByOrg(orgId);
  const humansById = useHumansByIds(humanIds);

  return useMemo(
    () =>
      humanIds
        .map((humanId) => humansById[humanId])
        .filter((human): human is Human => Boolean(human)),
    [humanIds, humansById],
  );
}

// --- composed reads -------------------------------------------------------

export function usePersonSessions(
  humanId: string | null | undefined,
): PersonSession[] {
  const mappingIds = main.UI.useSliceRowIds(
    main.INDEXES.sessionsByHuman,
    humanId ?? "",
    main.STORE_ID,
  );
  const allMappings = main.UI.useTable(
    "mapping_session_participant",
    main.STORE_ID,
  );
  const allSessions = main.UI.useTable("sessions", main.STORE_ID);

  return useMemo(() => {
    if (!mappingIds || mappingIds.length === 0) return [];
    const out: PersonSession[] = [];
    for (const mappingId of mappingIds) {
      const mapping = allMappings[mappingId];
      if (!mapping || !mapping.session_id || mapping.source === "excluded") {
        continue;
      }
      const sessionId = mapping.session_id as string;
      const session = allSessions[sessionId];
      if (!session) continue;
      out.push({
        id: sessionId,
        title: String(session.title ?? ""),
        created_at: String(session.created_at ?? ""),
      });
    }
    return out;
  }, [mappingIds, allMappings, allSessions]);
}

export function usePersonDuplicatesByEmail(
  humanId: string | null | undefined,
  email: string | undefined,
): PersonDuplicate[] {
  const duplicateIds = main.UI.useSliceRowIds(
    main.INDEXES.humansByEmail,
    email ?? "",
    main.STORE_ID,
  );
  const allHumans = main.UI.useTable("humans", main.STORE_ID);

  return useMemo(() => {
    if (!email || !duplicateIds || duplicateIds.length <= 1) return [];
    const out: PersonDuplicate[] = [];
    for (const id of duplicateIds) {
      if (id === humanId) continue;
      const row = allHumans[id];
      if (!row) continue;
      out.push({
        id,
        name: String(row.name ?? ""),
        email: String(row.email ?? ""),
      });
    }
    return out;
  }, [humanId, email, duplicateIds, allHumans]);
}

// --- writes ---------------------------------------------------------------

export function useUpdateHumanStringCell<K extends HumanStringField>(
  id: string,
  field: K,
): (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => void {
  const store = main.UI.useStore(main.STORE_ID);
  return useCallback(
    (e) => {
      if (!store) return;
      store.setPartialRow("humans", id, {
        [field]: e.target.value,
      } as Record<string, string>);
    },
    [store, id, field],
  );
}

export function useUpdateOrganizationStringCell<
  K extends OrganizationStringField,
>(
  id: string,
  field: K,
): (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => void {
  const store = main.UI.useStore(main.STORE_ID);
  return useCallback(
    (e) => {
      if (!store) return;
      store.setPartialRow("organizations", id, {
        [field]: e.target.value,
      } as Record<string, string>);
    },
    [store, id, field],
  );
}

export function useSetHumanOrgId(
  humanId: string,
): (orgId: string | null) => void {
  const store = main.UI.useStore(main.STORE_ID);
  return useCallback(
    (orgId) => {
      if (!store) return;
      store.setCell("humans", humanId, "org_id", orgId ?? "");
    },
    [store, humanId],
  );
}

export function useCreateHuman(): (args: {
  humanId: string;
  name: string;
}) => void {
  const userId = main.UI.useValue("user_id", main.STORE_ID);
  return main.UI.useSetRowCallback(
    "humans",
    (p: { humanId: string; name: string }) => p.humanId,
    (p: { humanId: string; name: string }) => ({
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
}

export function useCreateOrganization(): (args: {
  orgId: string;
  name: string;
}) => void {
  const userId = main.UI.useValue("user_id", main.STORE_ID);
  return main.UI.useSetRowCallback(
    "organizations",
    (p: { orgId: string; name: string }) => p.orgId,
    (p: { orgId: string; name: string }) => ({
      user_id: userId || "",
      name: p.name,
      created_at: new Date().toISOString(),
    }),
    [userId],
    main.STORE_ID,
  );
}

export function useDeleteHuman(): (id: string) => void {
  return main.UI.useDelRowCallback("humans", (id: string) => id, main.STORE_ID);
}

export function useDeleteOrganization(): (id: string) => void {
  return main.UI.useDelRowCallback(
    "organizations",
    (id: string) => id,
    main.STORE_ID,
  );
}

// --- composed writes ------------------------------------------------------

function nextPinOrder(
  humansTable: Record<string, Record<string, unknown>>,
  orgsTable: Record<string, Record<string, unknown>>,
): number {
  const maxH = Object.values(humansTable).reduce(
    (m, h) => Math.max(m, Number(h.pin_order ?? 0)),
    0,
  );
  const maxO = Object.values(orgsTable).reduce(
    (m, o) => Math.max(m, Number(o.pin_order ?? 0)),
    0,
  );
  return Math.max(maxH, maxO) + 1;
}

export function useToggleHumanPin(humanId: string): () => void {
  const store = main.UI.useStore(main.STORE_ID);
  return useCallback(() => {
    if (!store) return;
    const currentPinned = store.getCell("humans", humanId, "pinned");
    if (currentPinned) {
      store.setPartialRow("humans", humanId, {
        pinned: false,
        pin_order: 0,
      });
      return;
    }
    store.setPartialRow("humans", humanId, {
      pinned: true,
      pin_order: nextPinOrder(
        store.getTable("humans"),
        store.getTable("organizations"),
      ),
    });
  }, [store, humanId]);
}

export function useToggleOrganizationPin(orgId: string): () => void {
  const store = main.UI.useStore(main.STORE_ID);
  return useCallback(() => {
    if (!store) return;
    const currentPinned = store.getCell("organizations", orgId, "pinned");
    if (currentPinned) {
      store.setPartialRow("organizations", orgId, {
        pinned: false,
        pin_order: 0,
      });
      return;
    }
    store.setPartialRow("organizations", orgId, {
      pinned: true,
      pin_order: nextPinOrder(
        store.getTable("humans"),
        store.getTable("organizations"),
      ),
    });
  }, [store, orgId]);
}

export function useReorderPinnedContacts(): (
  ordered: PinnedContactItem[],
) => void {
  const store = main.UI.useStore(main.STORE_ID);
  return useCallback(
    (ordered) => {
      if (!store) return;
      store.transaction(() => {
        ordered.forEach((item, index) => {
          if (item.kind === "person") {
            store.setCell("humans", item.id, "pin_order", index);
          } else {
            store.setCell("organizations", item.id, "pin_order", index);
          }
        });
      });
    },
    [store],
  );
}

export function useMergeContacts(): (args: {
  primaryId: string;
  duplicateId: string;
}) => void {
  const store = main.UI.useStore(main.STORE_ID);
  return useCallback(
    ({ primaryId, duplicateId }) => {
      if (!store) return;

      const userId = store.getValue("user_id") as string;

      let keepId = primaryId;
      let dropId = duplicateId;
      if (duplicateId === userId) {
        keepId = duplicateId;
        dropId = primaryId;
      }

      const duplicateData = store.getRow("humans", dropId);
      const primaryData = store.getRow("humans", keepId);

      store.transaction(() => {
        const allMappingIds = store.getRowIds("mapping_session_participant");
        allMappingIds.forEach((mappingId) => {
          const mapping = store.getRow(
            "mapping_session_participant",
            mappingId,
          );
          if (mapping.human_id === dropId) {
            store.setPartialRow("mapping_session_participant", mappingId, {
              human_id: keepId,
            });
          }
        });

        if (duplicateData && primaryData) {
          const mergedFields: Record<string, string> = {};

          const concatField = (
            field: "job_title" | "linkedin_username" | "memo",
          ) => {
            const dup = duplicateData[field];
            const pri = primaryData[field];
            if (dup) {
              mergedFields[field] = pri ? `${pri}, ${dup}` : String(dup);
            }
          };
          concatField("job_title");
          concatField("linkedin_username");
          concatField("memo");

          if (!primaryData.org_id && duplicateData.org_id) {
            mergedFields.org_id = String(duplicateData.org_id);
          }

          if (Object.keys(mergedFields).length > 0) {
            store.setPartialRow("humans", keepId, mergedFields);
          }
        }

        store.delRow("humans", dropId);
      });
    },
    [store],
  );
}
