import { Trans, useLingui } from "@lingui/react/macro";
import {
  Building2,
  CircleMinus,
  FileText,
  Plus,
  SearchIcon,
} from "lucide-react";
import React, { useCallback, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { Textarea } from "@hypr/ui/components/ui/textarea";
import { cn } from "@hypr/utils";

import { ContactFacehash, getContactBgClass } from "./shared";

import * as main from "~/store/tinybase/store/main";

export function DetailsColumn({
  selectedHumanId,
  handleSessionClick,
}: {
  selectedHumanId?: string | null;
  handleSessionClick: (id: string) => void;
}) {
  const { t } = useLingui();
  const selectedPersonData = main.UI.useRow(
    "humans",
    selectedHumanId ?? "",
    main.STORE_ID,
  );
  const mappingIdsByHuman = main.UI.useSliceRowIds(
    main.INDEXES.sessionsByHuman,
    selectedHumanId ?? "",
    main.STORE_ID,
  );

  const allMappings = main.UI.useTable(
    "mapping_session_participant",
    main.STORE_ID,
  );
  const allSessions = main.UI.useTable("sessions", main.STORE_ID);

  const personSessions = React.useMemo(() => {
    if (!mappingIdsByHuman || mappingIdsByHuman.length === 0) {
      return [];
    }

    return mappingIdsByHuman
      .map((mappingId: string) => {
        const mapping = allMappings[mappingId];
        if (!mapping || !mapping.session_id || mapping.source === "excluded") {
          return null;
        }

        const sessionId = mapping.session_id as string;
        const session = allSessions[sessionId];
        if (!session) {
          return null;
        }

        return {
          id: sessionId,
          ...session,
        };
      })
      .filter(
        (session: any): session is NonNullable<typeof session> =>
          session !== null,
      );
  }, [mappingIdsByHuman, allMappings, allSessions]);

  const email = main.UI.useCell(
    "humans",
    selectedHumanId ?? "",
    "email",
    main.STORE_ID,
  ) as string | undefined;

  const duplicateHumanIds = main.UI.useSliceRowIds(
    main.INDEXES.humansByEmail,
    email ?? "",
    main.STORE_ID,
  );

  const duplicates = React.useMemo(() => {
    if (!email || !duplicateHumanIds || duplicateHumanIds.length <= 1) {
      return [];
    }
    return duplicateHumanIds.filter((id) => id !== selectedHumanId);
  }, [email, duplicateHumanIds, selectedHumanId]);

  const allHumans = main.UI.useTable("humans", main.STORE_ID);

  const duplicatesWithData = React.useMemo(() => {
    return duplicates
      .map((id) => {
        const data = allHumans[id];
        if (!data) return null;
        return { id, ...data };
      })
      .filter((dup): dup is NonNullable<typeof dup> => dup !== null);
  }, [duplicates, allHumans]);

  const store = main.UI.useStore(main.STORE_ID);

  const handleMergeContacts = useCallback(
    (duplicateId: string) => {
      if (!store || !selectedHumanId) return;

      const userId = store.getValue("user_id") as string;

      let primaryId = selectedHumanId;
      let dupId = duplicateId;

      if (duplicateId === userId) {
        primaryId = duplicateId;
        dupId = selectedHumanId;
      }

      const duplicateData = store.getRow("humans", dupId);
      const primaryData = store.getRow("humans", primaryId);

      store.transaction(() => {
        const allMappingIds = store.getRowIds("mapping_session_participant");
        allMappingIds.forEach((mappingId) => {
          const mapping = store.getRow(
            "mapping_session_participant",
            mappingId,
          );
          if (mapping.human_id === dupId) {
            store.setPartialRow("mapping_session_participant", mappingId, {
              human_id: primaryId,
            });
          }
        });

        if (duplicateData && primaryData) {
          const mergedFields: Record<string, any> = {};

          if (duplicateData.job_title) {
            if (primaryData.job_title) {
              mergedFields.job_title = `${primaryData.job_title}, ${duplicateData.job_title}`;
            } else {
              mergedFields.job_title = duplicateData.job_title;
            }
          }

          if (duplicateData.linkedin_username) {
            if (primaryData.linkedin_username) {
              mergedFields.linkedin_username = `${primaryData.linkedin_username}, ${duplicateData.linkedin_username}`;
            } else {
              mergedFields.linkedin_username = duplicateData.linkedin_username;
            }
          }

          if (duplicateData.phone) {
            if (primaryData.phone) {
              mergedFields.phone = `${primaryData.phone}, ${duplicateData.phone}`;
            } else {
              mergedFields.phone = duplicateData.phone;
            }
          }

          if (duplicateData.memo) {
            if (primaryData.memo) {
              mergedFields.memo = `${primaryData.memo}, ${duplicateData.memo}`;
            } else {
              mergedFields.memo = duplicateData.memo;
            }
          }

          if (!primaryData.org_id && duplicateData.org_id) {
            mergedFields.org_id = duplicateData.org_id;
          }

          if (Object.keys(mergedFields).length > 0) {
            store.setPartialRow("humans", primaryId, mergedFields);
          }
        }

        store.delRow("humans", dupId);
      });
    },
    [store, selectedHumanId],
  );

  const facehashName = String(
    selectedPersonData?.name ||
      selectedPersonData?.email ||
      selectedHumanId ||
      "",
  );
  const bgClass = getContactBgClass(facehashName);

  return (
    <div className="flex h-full flex-1 flex-col">
      {selectedPersonData && selectedHumanId ? (
        <>
          <div
            data-tauri-drag-region
            className="border-border flex items-center justify-center border-b py-6"
          >
            <div
              data-tauri-drag-region="false"
              className={cn(["rounded-full", bgClass])}
            >
              <ContactFacehash
                name={facehashName}
                size={64}
                interactive={true}
                showInitial={true}
                colorClasses={[bgClass]}
              />
            </div>
          </div>

          <div className="flex-1 overflow-y-auto">
            {duplicatesWithData.length > 0 && (
              <div className="border-border border-b bg-red-50 px-6 py-4">
                <h4 className="mb-1 text-sm font-semibold text-red-900">
                  Duplicate Contact
                  {duplicatesWithData.length > 1 ? "s" : ""} Found
                </h4>
                <p className="mb-3 text-sm text-red-800">
                  {duplicatesWithData.length > 1
                    ? `${duplicatesWithData.length} contacts`
                    : "Another contact"}{" "}
                  with the same email address{" "}
                  {duplicatesWithData.length > 1 ? "exist" : "exists"}. Merge to
                  consolidate all related notes and information.
                </p>
                <div className="flex flex-col gap-2">
                  {duplicatesWithData.map((dup) => (
                    <div
                      key={dup.id}
                      className="border-border bg-muted flex items-center justify-between rounded-md border p-2"
                    >
                      <div className="flex items-center gap-2">
                        <div
                          className={cn([
                            "shrink-0 rounded-full",
                            getContactBgClass(
                              String(dup.name || dup.email || dup.id),
                            ),
                          ])}
                        >
                          <ContactFacehash
                            name={String(dup.name || dup.email || dup.id)}
                            size={32}
                            interactive={false}
                            showInitial={false}
                            colorClasses={[
                              getContactBgClass(
                                String(dup.name || dup.email || dup.id),
                              ),
                            ]}
                          />
                        </div>
                        <div>
                          <div className="text-foreground text-sm font-medium">
                            {dup.name || "Unnamed Contact"}
                          </div>
                          <div className="text-muted-foreground text-xs">
                            {dup.email}
                          </div>
                        </div>
                      </div>
                      <Button
                        onClick={() => handleMergeContacts(dup.id)}
                        size="sm"
                        variant="default"
                      >
                        Merge
                      </Button>
                    </div>
                  ))}
                </div>
              </div>
            )}

            <div>
              <div className="border-border flex items-center border-b px-4 py-3">
                <div className="text-muted-foreground w-28 text-sm">
                  <Trans>Name</Trans>
                </div>
                <div className="flex-1">
                  <EditablePersonNameField personId={selectedHumanId} />
                </div>
              </div>
              <EditablePersonJobTitleField personId={selectedHumanId} />

              <div className="border-border flex items-center border-b px-4 py-3">
                <div className="text-muted-foreground w-28 text-sm">
                  <Trans>Company</Trans>
                </div>
                <div className="flex-1">
                  <EditPersonOrganizationSelector personId={selectedHumanId} />
                </div>
              </div>

              <EditablePersonEmailField personId={selectedHumanId} />
              <EditablePersonPhoneField personId={selectedHumanId} />
              <EditablePersonLinkedInField personId={selectedHumanId} />
              <EditablePersonMemoField personId={selectedHumanId} />
            </div>

            {personSessions.length > 0 && (
              <div className="border-border border-b p-6">
                <h3 className="text-muted-foreground mb-3 text-sm font-medium">
                  <Trans>Summary</Trans>
                </h3>
                <div className="border-border bg-muted rounded-lg border p-4">
                  <p className="text-muted-foreground text-sm leading-relaxed">
                    <Trans>
                      AI-generated summary of all interactions and notes with
                      this contact will appear here. This will synthesize key
                      discussion points, action items, and relationship context
                      across all meetings and notes.
                    </Trans>
                  </p>
                </div>
              </div>
            )}

            <div className="p-6">
              <h3 className="text-muted-foreground mb-4 text-sm font-medium">
                <Trans>Related Notes</Trans>
              </h3>
              <div className="flex flex-col gap-2">
                {personSessions.length > 0 ? (
                  personSessions.map((session: any) => (
                    <button
                      key={session.id}
                      onClick={() => handleSessionClick(session.id)}
                      className="border-border hover:bg-accent w-full rounded-md border p-3 text-left transition-colors"
                    >
                      <div className="mb-1 flex items-center gap-2">
                        <FileText className="text-muted-foreground h-4 w-4" />
                        <span className="text-sm font-medium">
                          {session.title || t`Untitled Note`}
                        </span>
                      </div>
                      {session.summary && (
                        <p className="text-muted-foreground mt-1 line-clamp-2 text-xs">
                          {session.summary}
                        </p>
                      )}
                      {session.created_at && (
                        <div className="text-muted-foreground mt-1 text-xs">
                          {new Date(session.created_at).toLocaleDateString()}
                        </div>
                      )}
                    </button>
                  ))
                ) : (
                  <p className="text-muted-foreground text-sm">
                    <Trans>No related notes found</Trans>
                  </p>
                )}
              </div>
            </div>

            <div className="pb-96" />
          </div>
        </>
      ) : (
        <div className="flex flex-1 items-center justify-center">
          <p className="text-muted-foreground text-sm">
            <Trans>Select a person to view details</Trans>
          </p>
        </div>
      )}
    </div>
  );
}

function EditablePersonNameField({ personId }: { personId: string }) {
  const { t } = useLingui();
  const value = main.UI.useCell("humans", personId, "name", main.STORE_ID);

  const handleChange = main.UI.useSetCellCallback(
    "humans",
    personId,
    "name",
    (e: React.ChangeEvent<HTMLInputElement>) => e.target.value,
    [],
    main.STORE_ID,
  );

  return (
    <Input
      value={(value as string) || ""}
      onChange={handleChange}
      placeholder={t`Name`}
      className="h-7 border-none p-0 text-base shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
    />
  );
}

function EditablePersonJobTitleField({ personId }: { personId: string }) {
  const { t } = useLingui();
  const value = main.UI.useCell("humans", personId, "job_title", main.STORE_ID);

  const handleChange = main.UI.useSetCellCallback(
    "humans",
    personId,
    "job_title",
    (e: React.ChangeEvent<HTMLInputElement>) => e.target.value,
    [],
    main.STORE_ID,
  );

  return (
    <div className="border-border flex items-center border-b px-4 py-3">
      <div className="text-muted-foreground w-28 text-sm">
        <Trans>Job Title</Trans>
      </div>
      <div className="flex-1">
        <Input
          value={(value as string) || ""}
          onChange={handleChange}
          placeholder={t`Software Engineer`}
          className="h-7 border-none p-0 text-base shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
        />
      </div>
    </div>
  );
}

function EditablePersonEmailField({ personId }: { personId: string }) {
  const value = main.UI.useCell("humans", personId, "email", main.STORE_ID);

  const handleChange = main.UI.useSetCellCallback(
    "humans",
    personId,
    "email",
    (e: React.ChangeEvent<HTMLInputElement>) => e.target.value,
    [],
    main.STORE_ID,
  );

  return (
    <div className="border-border flex items-center border-b px-4 py-3">
      <div className="text-muted-foreground w-28 text-sm">
        <Trans>Email</Trans>
      </div>
      <div className="flex-1">
        <Input
          type="email"
          value={(value as string) || ""}
          onChange={handleChange}
          placeholder="john@example.com"
          className="h-7 border-none p-0 text-base shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
        />
      </div>
    </div>
  );
}

function EditablePersonPhoneField({ personId }: { personId: string }) {
  const value = main.UI.useCell("humans", personId, "phone", main.STORE_ID);

  const handleChange = main.UI.useSetCellCallback(
    "humans",
    personId,
    "phone",
    (e: React.ChangeEvent<HTMLInputElement>) => e.target.value,
    [],
    main.STORE_ID,
  );

  return (
    <div className="border-border flex items-center border-b px-4 py-3">
      <div className="text-muted-foreground w-28 text-sm">
        <Trans>Phone</Trans>
      </div>
      <div className="flex-1">
        <Input
          type="tel"
          value={(value as string) || ""}
          onChange={handleChange}
          placeholder="+1 (555) 123-4567"
          className="h-7 border-none p-0 text-base shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
        />
      </div>
    </div>
  );
}

function EditablePersonLinkedInField({ personId }: { personId: string }) {
  const value = main.UI.useCell(
    "humans",
    personId,
    "linkedin_username",
    main.STORE_ID,
  );

  const handleChange = main.UI.useSetCellCallback(
    "humans",
    personId,
    "linkedin_username",
    (e: React.ChangeEvent<HTMLInputElement>) => e.target.value,
    [],
    main.STORE_ID,
  );

  return (
    <div className="border-border flex items-center border-b px-4 py-3">
      <div className="text-muted-foreground w-28 text-sm">
        <Trans>LinkedIn</Trans>
      </div>
      <div className="flex-1">
        <Input
          value={(value as string) || ""}
          onChange={handleChange}
          placeholder="https://www.linkedin.com/in/johntopia/"
          className="h-7 border-none p-0 text-base shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
        />
      </div>
    </div>
  );
}

function EditablePersonMemoField({ personId }: { personId: string }) {
  const { t } = useLingui();
  const value = main.UI.useCell("humans", personId, "memo", main.STORE_ID);

  const handleChange = main.UI.useSetCellCallback(
    "humans",
    personId,
    "memo",
    (e: React.ChangeEvent<HTMLTextAreaElement>) => e.target.value,
    [],
    main.STORE_ID,
  );

  return (
    <div className="border-border flex border-b px-4 py-3">
      <div className="text-muted-foreground w-28 pt-2 text-sm">
        <Trans>Notes</Trans>
      </div>
      <div className="flex-1">
        <Textarea
          value={(value as string) || ""}
          onChange={handleChange}
          placeholder={t`Add notes about this contact...`}
          className="min-h-[80px] resize-none border-none px-0 py-2 text-base shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
          rows={3}
        />
      </div>
    </div>
  );
}

function EditPersonOrganizationSelector({ personId }: { personId: string }) {
  const [open, setOpen] = useState(false);
  const orgId = main.UI.useCell("humans", personId, "org_id", main.STORE_ID) as
    | string
    | null;
  const organization = main.UI.useRow(
    "organizations",
    orgId ?? "",
    main.STORE_ID,
  );

  const handleChange = main.UI.useSetCellCallback(
    "humans",
    personId,
    "org_id",
    (newOrgId: string | null) => newOrgId ?? "",
    [],
    main.STORE_ID,
  );

  const handleRemoveOrganization = () => {
    handleChange(null);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <div className="hover:bg-accent -mx-2 inline-flex cursor-pointer items-center rounded-lg px-2 py-1 transition-colors">
          {organization?.name ? (
            <div className="flex items-center">
              <span className="text-base">{organization.name}</span>
              <span className="group text-muted-foreground ml-2">
                <CircleMinus
                  className="text-muted-foreground size-4 cursor-pointer hover:text-red-600"
                  onClick={(e) => {
                    e.stopPropagation();
                    handleRemoveOrganization();
                  }}
                />
              </span>
            </div>
          ) : (
            <span className="text-muted-foreground flex items-center gap-1 text-base">
              <Plus className="size-4" />
              <Trans>Add organization</Trans>
            </span>
          )}
        </div>
      </PopoverTrigger>

      <PopoverContent variant="app" align="start" side="bottom">
        <AppFloatingPanel className="p-3">
          <OrganizationControl
            onChange={handleChange}
            closePopover={() => setOpen(false)}
          />
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}

function OrganizationControl({
  onChange,
  closePopover,
}: {
  onChange: (orgId: string | null) => void;
  closePopover: () => void;
}) {
  const { t } = useLingui();
  const [searchTerm, setSearchTerm] = useState("");
  const [highlightedIndex, setHighlightedIndex] = useState(-1);
  const userId = main.UI.useValue("user_id", main.STORE_ID);

  const organizationsData = main.UI.useResultTable(
    main.QUERIES.visibleOrganizations,
    main.STORE_ID,
  );

  const allOrganizations = Object.entries(organizationsData).map(
    ([id, data]) => ({
      id,
      ...(data as any),
    }),
  );

  const organizations = searchTerm.trim()
    ? allOrganizations.filter((org: any) =>
        org.name.toLowerCase().includes(searchTerm.toLowerCase()),
      )
    : allOrganizations;

  const showCreateOption = searchTerm.trim() && organizations.length === 0;
  const itemCount = organizations.length + (showCreateOption ? 1 : 0);

  const createOrganization = main.UI.useSetRowCallback(
    "organizations",
    (p: { name: string; orgId: string }) => p.orgId,
    (p: { name: string; orgId: string }) => ({
      user_id: userId || "",
      name: p.name,
      created_at: new Date().toISOString(),
    }),
    [userId],
    main.STORE_ID,
  );

  const handleCreateOrganization = () => {
    const orgId = crypto.randomUUID();
    createOrganization({ orgId, name: searchTerm.trim() });
    onChange(orgId);
    closePopover();
  };

  const handleSubmit = (e: React.SyntheticEvent<HTMLFormElement>) => {
    e.preventDefault();
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setHighlightedIndex((prev) => (prev < itemCount - 1 ? prev + 1 : 0));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setHighlightedIndex((prev) => (prev > 0 ? prev - 1 : itemCount - 1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (highlightedIndex >= 0 && highlightedIndex < organizations.length) {
        selectOrganization(organizations[highlightedIndex].id);
      } else if (showCreateOption) {
        handleCreateOrganization();
      }
    }
  };

  const selectOrganization = (orgId: string) => {
    onChange(orgId);
    closePopover();
  };

  return (
    <div className="flex max-w-[450px] flex-col gap-3">
      <div className="text-muted-foreground text-sm font-medium">
        <Trans>Organization</Trans>
      </div>

      <form onSubmit={handleSubmit}>
        <div className="flex flex-col gap-2">
          <div className="border-border bg-muted flex w-full items-center gap-2 rounded-xs border px-2 py-1.5">
            <span className="text-muted-foreground shrink-0">
              <SearchIcon className="size-4" />
            </span>
            <input
              type="text"
              value={searchTerm}
              onChange={(e) => {
                setSearchTerm(e.target.value);
                setHighlightedIndex(-1);
              }}
              onKeyDown={handleKeyDown}
              placeholder={t`Search or add company`}
              className="placeholder:text-muted-foreground w-full bg-transparent text-sm focus:outline-hidden focus-visible:ring-0 focus-visible:ring-offset-0"
            />
          </div>

          {searchTerm.trim() && (
            <div className="border-border flex w-full flex-col overflow-hidden rounded-xs border">
              {organizations.map((org: any, index: number) => (
                <button
                  key={org.id}
                  type="button"
                  className={[
                    "flex items-center px-3 py-2 text-sm text-left transition-colors w-full",
                    highlightedIndex === index ? "bg-muted" : "hover:bg-accent",
                  ].join(" ")}
                  onClick={() => selectOrganization(org.id)}
                  onMouseEnter={() => setHighlightedIndex(index)}
                >
                  <span className="bg-muted mr-2 flex size-5 shrink-0 items-center justify-center rounded-full">
                    <Building2 className="size-3" />
                  </span>
                  <span className="truncate font-medium">{org.name}</span>
                </button>
              ))}

              {showCreateOption && (
                <button
                  type="button"
                  className={[
                    "flex items-center px-3 py-2 text-sm text-left transition-colors w-full",
                    highlightedIndex === organizations.length
                      ? "bg-muted"
                      : "hover:bg-accent",
                  ].join(" ")}
                  onClick={() => handleCreateOrganization()}
                  onMouseEnter={() => setHighlightedIndex(organizations.length)}
                >
                  <span className="bg-accent mr-2 flex size-5 shrink-0 items-center justify-center rounded-full">
                    <span className="text-xs">+</span>
                  </span>
                  <span className="text-muted-foreground flex items-center gap-1 font-medium">
                    Create
                    <span className="text-foreground max-w-[140px] truncate">
                      &quot;{searchTerm.trim()}&quot;
                    </span>
                  </span>
                </button>
              )}
            </div>
          )}

          {!searchTerm.trim() && organizations.length > 0 && (
            <div className="custom-scrollbar border-border flex max-h-[40vh] w-full flex-col overflow-hidden overflow-y-auto rounded-xs border">
              {organizations.map((org: any, index: number) => (
                <button
                  key={org.id}
                  type="button"
                  className={[
                    "flex items-center px-3 py-2 text-sm text-left transition-colors w-full",
                    highlightedIndex === index ? "bg-muted" : "hover:bg-accent",
                  ].join(" ")}
                  onClick={() => selectOrganization(org.id)}
                  onMouseEnter={() => setHighlightedIndex(index)}
                >
                  <span className="bg-muted mr-2 flex size-5 shrink-0 items-center justify-center rounded-full">
                    <Building2 className="size-3" />
                  </span>
                  <span className="truncate font-medium">{org.name}</span>
                </button>
              ))}
            </div>
          )}
        </div>
      </form>
    </div>
  );
}
