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

import {
  createOrganization,
  type HumanRecord,
  mergeHumans,
  type OrganizationRecord,
  updateHuman,
  useHumanSessions,
} from "./queries";
import { ContactFacehash, getContactBgClass } from "./shared";

export function DetailsColumn({
  human,
  humans,
  organizations,
  handleSessionClick,
}: {
  human: HumanRecord | null;
  humans: HumanRecord[];
  organizations: OrganizationRecord[];
  handleSessionClick: (id: string) => void;
}) {
  const { t } = useLingui();
  const personSessions = useHumanSessions(human?.id ?? "");
  const duplicatesWithData = React.useMemo(
    () =>
      human?.email
        ? humans.filter(
            (candidate) =>
              candidate.id !== human.id && candidate.email === human.email,
          )
        : [],
    [human, humans],
  );

  const handleMergeContacts = useCallback(
    (duplicateId: string) => {
      if (!human) return;
      void mergeHumans(human.id, duplicateId).catch((error) => {
        console.error("[contacts] failed to merge contacts", error);
      });
    },
    [human],
  );

  const facehashName = String(human?.name || human?.email || human?.id || "");
  const bgClass = getContactBgClass(facehashName);

  return (
    <div className="flex h-full flex-1 flex-col">
      {human ? (
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
                  <EditablePersonNameField
                    key={`${human.id}:name`}
                    personId={human.id}
                    value={human.name}
                  />
                </div>
              </div>
              <EditablePersonJobTitleField
                key={`${human.id}:job-title`}
                personId={human.id}
                value={human.jobTitle}
              />

              <div className="border-border flex items-center border-b px-4 py-3">
                <div className="text-muted-foreground w-28 text-sm">
                  <Trans>Company</Trans>
                </div>
                <div className="flex-1">
                  <EditPersonOrganizationSelector
                    personId={human.id}
                    organization={
                      organizations.find(
                        (organization) =>
                          organization.id === human.organizationId,
                      ) ?? null
                    }
                    organizations={organizations}
                  />
                </div>
              </div>

              <EditablePersonEmailField
                key={`${human.id}:email`}
                personId={human.id}
                value={human.email}
              />
              <EditablePersonPhoneField
                key={`${human.id}:phone`}
                personId={human.id}
                value={human.phone}
              />
              <EditablePersonLinkedInField
                key={`${human.id}:linkedin`}
                personId={human.id}
                value={human.linkedinUsername}
              />
              <EditablePersonMemoField
                key={`${human.id}:memo`}
                personId={human.id}
                value={human.memo}
              />
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
                  personSessions.map((session) => (
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
                      {session.createdAt && (
                        <div className="text-muted-foreground mt-1 text-xs">
                          {new Date(session.createdAt).toLocaleDateString()}
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

function EditablePersonNameField({
  personId,
  value,
}: {
  personId: string;
  value: string;
}) {
  const { t } = useLingui();

  return (
    <Input
      defaultValue={value}
      onChange={(event) =>
        persistHumanUpdate(personId, { name: event.target.value })
      }
      placeholder={t`Name`}
      className="h-7 border-none p-0 text-base shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
    />
  );
}

function EditablePersonJobTitleField({
  personId,
  value,
}: {
  personId: string;
  value: string;
}) {
  const { t } = useLingui();

  return (
    <div className="border-border flex items-center border-b px-4 py-3">
      <div className="text-muted-foreground w-28 text-sm">
        <Trans>Job Title</Trans>
      </div>
      <div className="flex-1">
        <Input
          defaultValue={value}
          onChange={(event) =>
            persistHumanUpdate(personId, { jobTitle: event.target.value })
          }
          placeholder={t`Software Engineer`}
          className="h-7 border-none p-0 text-base shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
        />
      </div>
    </div>
  );
}

function EditablePersonEmailField({
  personId,
  value,
}: {
  personId: string;
  value: string;
}) {
  return (
    <div className="border-border flex items-center border-b px-4 py-3">
      <div className="text-muted-foreground w-28 text-sm">
        <Trans>Email</Trans>
      </div>
      <div className="flex-1">
        <Input
          type="email"
          defaultValue={value}
          onChange={(event) =>
            persistHumanUpdate(personId, { email: event.target.value })
          }
          placeholder="john@example.com"
          className="h-7 border-none p-0 text-base shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
        />
      </div>
    </div>
  );
}

function EditablePersonPhoneField({
  personId,
  value,
}: {
  personId: string;
  value: string;
}) {
  return (
    <div className="border-border flex items-center border-b px-4 py-3">
      <div className="text-muted-foreground w-28 text-sm">
        <Trans>Phone</Trans>
      </div>
      <div className="flex-1">
        <Input
          type="tel"
          defaultValue={value}
          onChange={(event) =>
            persistHumanUpdate(personId, { phone: event.target.value })
          }
          placeholder="+1 (555) 123-4567"
          className="h-7 border-none p-0 text-base shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
        />
      </div>
    </div>
  );
}

function EditablePersonLinkedInField({
  personId,
  value,
}: {
  personId: string;
  value: string;
}) {
  return (
    <div className="border-border flex items-center border-b px-4 py-3">
      <div className="text-muted-foreground w-28 text-sm">
        <Trans>LinkedIn</Trans>
      </div>
      <div className="flex-1">
        <Input
          defaultValue={value}
          onChange={(event) =>
            persistHumanUpdate(personId, {
              linkedinUsername: event.target.value,
            })
          }
          placeholder="https://www.linkedin.com/in/johntopia/"
          className="h-7 border-none p-0 text-base shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
        />
      </div>
    </div>
  );
}

function EditablePersonMemoField({
  personId,
  value,
}: {
  personId: string;
  value: string;
}) {
  const { t } = useLingui();

  return (
    <div className="border-border flex border-b px-4 py-3">
      <div className="text-muted-foreground w-28 pt-2 text-sm">
        <Trans>Notes</Trans>
      </div>
      <div className="flex-1">
        <Textarea
          defaultValue={value}
          onChange={(event) =>
            persistHumanUpdate(personId, { memo: event.target.value })
          }
          placeholder={t`Add notes about this contact...`}
          className="min-h-[80px] resize-none border-none px-0 py-2 text-base shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
          rows={3}
        />
      </div>
    </div>
  );
}

function EditPersonOrganizationSelector({
  personId,
  organization,
  organizations,
}: {
  personId: string;
  organization: OrganizationRecord | null;
  organizations: OrganizationRecord[];
}) {
  const [open, setOpen] = useState(false);
  const handleChange = (organizationId: string | null) => {
    persistHumanUpdate(personId, {
      organizationId: organizationId ?? "",
    });
  };

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
            organizations={organizations}
            onChange={handleChange}
            closePopover={() => setOpen(false)}
          />
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}

function OrganizationControl({
  organizations: allOrganizations,
  onChange,
  closePopover,
}: {
  organizations: OrganizationRecord[];
  onChange: (orgId: string | null) => void;
  closePopover: () => void;
}) {
  const { t } = useLingui();
  const [searchTerm, setSearchTerm] = useState("");
  const [highlightedIndex, setHighlightedIndex] = useState(-1);

  const organizations = searchTerm.trim()
    ? allOrganizations.filter((org) =>
        org.name.toLowerCase().includes(searchTerm.toLowerCase()),
      )
    : allOrganizations;

  const showCreateOption = searchTerm.trim() && organizations.length === 0;
  const itemCount = organizations.length + (showCreateOption ? 1 : 0);

  const handleCreateOrganization = async () => {
    try {
      const organizationId = await createOrganization({
        name: searchTerm.trim(),
      });
      onChange(organizationId);
      closePopover();
    } catch (error) {
      console.error("[contacts] failed to create organization", error);
    }
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
        void handleCreateOrganization();
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
              {organizations.map((org, index) => (
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
                  onClick={() => void handleCreateOrganization()}
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
              {organizations.map((org, index) => (
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

function persistHumanUpdate(
  personId: string,
  changes: Parameters<typeof updateHuman>[1],
): void {
  void updateHuman(personId, changes).catch((error) => {
    console.error("[contacts] failed to update contact", error);
  });
}
