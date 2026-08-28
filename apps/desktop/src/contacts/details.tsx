import { Trans, useLingui } from "@lingui/react/macro";
import {
  Buildings,
  CircleNotch,
  MagnifyingGlass,
  MinusCircle,
  Plus,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import React, { useCallback, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import { Textarea } from "@anlg/ui/components/ui/textarea";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import {
  AvatarUploadButton,
  ContactImage,
  persistContactAvatar,
} from "./contact-avatar";
import { ContactPageHeader } from "./contact-page-header";
import { useContactSummary } from "./contact-summary";
import {
  createOrganization,
  type HumanRecord,
  mergeHumans,
  type OrganizationRecord,
  toggleContactPin,
  updateHuman,
  useHumanSessions,
} from "./queries";
import { RelatedNotesSection } from "./related-notes";
import { ContactFacehash } from "./shared";

const SUMMARY_SKELETON_WIDTHS = ["80%", "66.666667%", "60%"];

export function DetailsColumn({
  human,
  humans,
  organizations,
  handleSessionClick,
  onDelete,
}: {
  human: HumanRecord | null;
  humans: HumanRecord[];
  organizations: OrganizationRecord[];
  handleSessionClick: (id: string) => void;
  onDelete: (id: string) => void;
}) {
  const { t } = useLingui();
  const [showCompactIdentity, setShowCompactIdentity] = useState(false);
  const personSessions = useHumanSessions(human?.id ?? "");
  const organizationName =
    organizations.find(
      (organization) => organization.id === human?.organizationId,
    )?.name ?? null;
  const contactSummary = useContactSummary({
    human,
    organizationName,
    sessions: personSessions,
  });
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

  return (
    <div {...stylex.props(styles.root)}>
      {human ? (
        <>
          <ContactPageHeader
            title={human.name || human.email || t`Unnamed`}
            compactIdentity={
              human.avatarDataUrl ? (
                <ContactImage src={human.avatarDataUrl} size={24} />
              ) : (
                <ContactFacehash name={facehashName} size={24} />
              )
            }
            showCompactIdentity={showCompactIdentity}
            pinned={Boolean(human.pinned)}
            onTogglePin={() => {
              void toggleContactPin("human", human.id).catch((error) => {
                console.error("[contacts] failed to toggle contact pin", error);
              });
            }}
            onDelete={() => onDelete(human.id)}
            onRemoveAvatar={
              human.avatarDataUrl
                ? () => persistContactAvatar("human", human.id, null)
                : undefined
            }
          />

          <div
            {...stylex.props(styles.scroller)}
            onScroll={(event) => {
              setShowCompactIdentity(event.currentTarget.scrollTop > 0);
            }}
          >
            <div {...stylex.props(styles.avatarSection)}>
              <AvatarUploadButton
                label={t`Change photo`}
                onUpload={(dataUrl) =>
                  persistContactAvatar("human", human.id, dataUrl)
                }
              >
                {human.avatarDataUrl ? (
                  <ContactImage src={human.avatarDataUrl} size={64} />
                ) : (
                  <ContactFacehash name={facehashName} size={64} />
                )}
              </AvatarUploadButton>
            </div>

            {duplicatesWithData.length > 0 && (
              <div {...stylex.props(styles.duplicateAlert)}>
                <h4 {...stylex.props(styles.duplicateTitle)}>
                  Duplicate Contact
                  {duplicatesWithData.length > 1 ? "s" : ""} Found
                </h4>
                <p {...stylex.props(styles.duplicateDescription)}>
                  {duplicatesWithData.length > 1
                    ? `${duplicatesWithData.length} contacts`
                    : "Another contact"}{" "}
                  with the same email address{" "}
                  {duplicatesWithData.length > 1 ? "exist" : "exists"}. Merge to
                  consolidate all related notes and information.
                </p>
                <div {...stylex.props(styles.duplicateList)}>
                  {duplicatesWithData.map((dup) => (
                    <div key={dup.id} {...stylex.props(styles.duplicate)}>
                      <div {...stylex.props(styles.duplicateIdentity)}>
                        {dup.avatarDataUrl ? (
                          <ContactImage src={dup.avatarDataUrl} size={32} />
                        ) : (
                          <ContactFacehash
                            name={String(dup.name || dup.email || dup.id)}
                            size={32}
                          />
                        )}
                        <div>
                          <div {...stylex.props(styles.duplicateName)}>
                            {dup.name || "Unnamed Contact"}
                          </div>
                          <div {...stylex.props(styles.duplicateEmail)}>
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
              <div {...stylex.props(styles.fieldRow)}>
                <div {...stylex.props(styles.fieldLabel)}>
                  <Trans>Name</Trans>
                </div>
                <div {...stylex.props(styles.fieldControl)}>
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

              <div {...stylex.props(styles.fieldRow)}>
                <div {...stylex.props(styles.fieldLabel)}>
                  <Trans>Company</Trans>
                </div>
                <div {...stylex.props(styles.fieldControl)}>
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
              <ContactSummarySection summary={contactSummary} />
            )}

            <RelatedNotesSection
              sessions={personSessions}
              onSessionClick={handleSessionClick}
            />

            <div {...stylex.props(styles.scrollSpacer)} />
          </div>
        </>
      ) : (
        <div {...stylex.props(styles.emptyState)}>
          <p {...stylex.props(styles.mutedText)}>
            <Trans>Select a person to view details</Trans>
          </p>
        </div>
      )}
    </div>
  );
}

function ContactSummarySection({
  summary,
}: {
  summary: ReturnType<typeof useContactSummary>;
}) {
  const hasFacts = summary.facts.length > 0;

  return (
    <div {...stylex.props(styles.summarySection)}>
      <div {...stylex.props(styles.summaryHeader)}>
        <h3 {...stylex.props(styles.summaryHeading)}>
          <Trans>Summary</Trans>
        </h3>
        {summary.isGenerating && (
          <>
            <CircleNotch
              aria-hidden="true"
              {...stylex.props(styles.summarySpinner)}
            />
            <span {...stylex.props(styles.visuallyHidden)}>
              <Trans>Loading...</Trans>
            </span>
          </>
        )}
      </div>

      <div {...stylex.props(styles.summaryCard)}>
        {hasFacts ? (
          <ul {...stylex.props(styles.facts)}>
            {summary.facts.map((fact) => (
              <li key={fact}>{fact}</li>
            ))}
          </ul>
        ) : summary.isGenerating ? (
          <div aria-hidden="true" {...stylex.props(styles.skeleton)}>
            {SUMMARY_SKELETON_WIDTHS.map((width, index) => (
              <div key={width} {...stylex.props(styles.skeletonRow)}>
                <div
                  {...stylex.props([
                    styles.skeletonBullet,
                    styles.animationDelay(index * 150),
                  ])}
                />
                <div
                  {...stylex.props([
                    styles.skeletonLine,
                    styles.skeletonWidth(width),
                    styles.animationDelay(index * 150),
                  ])}
                />
              </div>
            ))}
          </div>
        ) : summary.error ? (
          <div {...stylex.props(styles.summaryError)}>
            <p {...stylex.props(styles.mutedText)}>
              <Trans>Summary generation failed</Trans>
            </p>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => void summary.retry()}
            >
              <Trans>Try again</Trans>
            </Button>
          </div>
        ) : (
          <p {...stylex.props([styles.mutedText, styles.relaxedText])}>
            <Trans>
              AI-generated summary of all interactions and notes with this
              contact will appear here. This will synthesize key discussion
              points, action items, and relationship context across all meetings
              and notes.
            </Trans>
          </p>
        )}

        {hasFacts && summary.error && (
          <div {...stylex.props(styles.partialSummaryError)}>
            <p {...stylex.props(styles.caption)}>
              <Trans>Summary generation failed</Trans>
            </p>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => void summary.retry()}
            >
              <Trans>Try again</Trans>
            </Button>
          </div>
        )}
      </div>
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
      sx={styles.fieldInput}
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
    <div {...stylex.props(styles.fieldRow)}>
      <div {...stylex.props(styles.fieldLabel)}>
        <Trans>Job Title</Trans>
      </div>
      <div {...stylex.props(styles.fieldControl)}>
        <Input
          defaultValue={value}
          onChange={(event) =>
            persistHumanUpdate(personId, { jobTitle: event.target.value })
          }
          placeholder={t`Software Engineer`}
          sx={styles.fieldInput}
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
    <div {...stylex.props(styles.fieldRow)}>
      <div {...stylex.props(styles.fieldLabel)}>
        <Trans>Email</Trans>
      </div>
      <div {...stylex.props(styles.fieldControl)}>
        <Input
          type="email"
          defaultValue={value}
          onChange={(event) =>
            persistHumanUpdate(personId, { email: event.target.value })
          }
          placeholder="john@example.com"
          sx={styles.fieldInput}
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
    <div {...stylex.props(styles.fieldRow)}>
      <div {...stylex.props(styles.fieldLabel)}>
        <Trans>Phone</Trans>
      </div>
      <div {...stylex.props(styles.fieldControl)}>
        <Input
          type="tel"
          defaultValue={value}
          onChange={(event) =>
            persistHumanUpdate(personId, { phone: event.target.value })
          }
          placeholder="+1 (555) 123-4567"
          sx={styles.fieldInput}
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
    <div {...stylex.props(styles.fieldRow)}>
      <div {...stylex.props(styles.fieldLabel)}>
        <Trans>LinkedIn</Trans>
      </div>
      <div {...stylex.props(styles.fieldControl)}>
        <Input
          defaultValue={value}
          onChange={(event) =>
            persistHumanUpdate(personId, {
              linkedinUsername: event.target.value,
            })
          }
          placeholder="https://www.linkedin.com/in/johntopia/"
          sx={styles.fieldInput}
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
    <div {...stylex.props([styles.fieldRow, styles.memoRow])}>
      <div {...stylex.props([styles.fieldLabel, styles.memoLabel])}>
        <Trans>Notes</Trans>
      </div>
      <div {...stylex.props(styles.fieldControl)}>
        <Textarea
          defaultValue={value}
          onChange={(event) =>
            persistHumanUpdate(personId, { memo: event.target.value })
          }
          placeholder={t`Add notes about this contact...`}
          sx={styles.memoInput}
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
        <div {...stylex.props(styles.organizationTrigger)}>
          {organization?.name ? (
            <div {...stylex.props(styles.selectedOrganization)}>
              <span {...stylex.props(styles.organizationName)}>
                {organization.name}
              </span>
              <span {...stylex.props(styles.removeOrganizationContainer)}>
                <MinusCircle
                  {...stylex.props(styles.removeOrganization)}
                  onClick={(e) => {
                    e.stopPropagation();
                    handleRemoveOrganization();
                  }}
                />
              </span>
            </div>
          ) : (
            <span {...stylex.props(styles.addOrganization)}>
              <Plus {...stylex.props(styles.icon)} />
              <Trans>Add organization</Trans>
            </span>
          )}
        </div>
      </PopoverTrigger>

      <PopoverContent variant="app" align="start" side="bottom">
        <AppFloatingPanel sx={styles.organizationPanel}>
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
    <div {...stylex.props(styles.organizationControl)}>
      <div {...stylex.props(styles.organizationHeading)}>
        <Trans>Organization</Trans>
      </div>

      <form onSubmit={handleSubmit}>
        <div {...stylex.props(styles.organizationForm)}>
          <div {...stylex.props(styles.organizationSearch)}>
            <span {...stylex.props(styles.organizationSearchIcon)}>
              <MagnifyingGlass {...stylex.props(styles.icon)} />
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
              {...stylex.props(styles.organizationSearchInput)}
            />
          </div>

          {searchTerm.trim() && (
            <div {...stylex.props(styles.organizationList)}>
              {organizations.map((org, index) => (
                <button
                  key={org.id}
                  type="button"
                  {...stylex.props([
                    styles.organizationOption,
                    highlightedIndex === index
                      ? styles.highlightedOption
                      : styles.unhighlightedOption,
                  ])}
                  onClick={() => selectOrganization(org.id)}
                  onMouseEnter={() => setHighlightedIndex(index)}
                >
                  <span {...stylex.props(styles.organizationOptionIcon)}>
                    <Buildings {...stylex.props(styles.smallIcon)} />
                  </span>
                  <span {...stylex.props(styles.organizationOptionName)}>
                    {org.name}
                  </span>
                </button>
              ))}

              {showCreateOption && (
                <button
                  type="button"
                  {...stylex.props([
                    styles.organizationOption,
                    highlightedIndex === organizations.length
                      ? styles.highlightedOption
                      : styles.unhighlightedOption,
                  ])}
                  onClick={() => void handleCreateOrganization()}
                  onMouseEnter={() => setHighlightedIndex(organizations.length)}
                >
                  <span
                    {...stylex.props([
                      styles.organizationOptionIcon,
                      styles.createOptionIcon,
                    ])}
                  >
                    <span {...stylex.props(styles.caption)}>+</span>
                  </span>
                  <span {...stylex.props(styles.createOptionLabel)}>
                    Create
                    <span {...stylex.props(styles.createOptionName)}>
                      &quot;{searchTerm.trim()}&quot;
                    </span>
                  </span>
                </button>
              )}
            </div>
          )}

          {!searchTerm.trim() && organizations.length > 0 && (
            <div
              {...mergeStyleXProps(
                styles.organizationListScrollable,
                "custom-scrollbar",
              )}
            >
              {organizations.map((org, index) => (
                <button
                  key={org.id}
                  type="button"
                  {...stylex.props([
                    styles.organizationOption,
                    highlightedIndex === index
                      ? styles.highlightedOption
                      : styles.unhighlightedOption,
                  ])}
                  onClick={() => selectOrganization(org.id)}
                  onMouseEnter={() => setHighlightedIndex(index)}
                >
                  <span {...stylex.props(styles.organizationOptionIcon)}>
                    <Buildings {...stylex.props(styles.smallIcon)} />
                  </span>
                  <span {...stylex.props(styles.organizationOptionName)}>
                    {org.name}
                  </span>
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

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const pulse = stylex.keyframes({
  "0%, 100%": {
    opacity: 1,
  },
  "50%": {
    opacity: 0.5,
  },
});

const styles = stylex.create({
  addOrganization: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "1rem",
    gap: "0.25rem",
    lineHeight: "1.5rem",
  },
  animationDelay: (delay: number) => ({
    animationDelay: `${delay}ms`,
  }),
  avatarSection: {
    alignItems: "center",
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    display: "flex",
    justifyContent: "center",
    paddingBlock: "1.5rem",
  },
  caption: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  createOptionIcon: {
    backgroundColor: colors.accent,
  },
  createOptionLabel: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontWeight: 500,
    gap: "0.25rem",
  },
  createOptionName: {
    color: colors.foreground,
    maxWidth: "140px",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  duplicate: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    justifyContent: "space-between",
    padding: "0.5rem",
  },
  duplicateAlert: {
    backgroundColor: "rgb(254 242 242)",
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    paddingBlock: "1rem",
    paddingInline: "1.5rem",
  },
  duplicateDescription: {
    color: "rgb(153 27 27)",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    marginBottom: "0.75rem",
  },
  duplicateEmail: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  duplicateIdentity: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  duplicateList: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  duplicateName: {
    color: colors.foreground,
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
  },
  duplicateTitle: {
    color: "rgb(127 29 29)",
    fontSize: "0.875rem",
    fontWeight: 600,
    lineHeight: "1.25rem",
    marginBottom: "0.25rem",
  },
  emptyState: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    justifyContent: "center",
  },
  facts: {
    color: colors.foreground,
    display: "flex",
    flexDirection: "column",
    fontSize: "0.875rem",
    gap: "0.5rem",
    lineHeight: 1.625,
    listStyleType: "disc",
    paddingLeft: "1.25rem",
  },
  fieldControl: {
    flex: "1",
  },
  fieldInput: {
    borderWidth: 0,
    boxShadow: {
      default: "none",
      ":focus-visible": "none",
    },
    fontSize: "1rem",
    height: "1.75rem",
    padding: 0,
  },
  fieldLabel: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    width: "7rem",
  },
  fieldRow: {
    alignItems: "center",
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    display: "flex",
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  highlightedOption: {
    backgroundColor: colors.muted,
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  memoInput: {
    borderWidth: 0,
    boxShadow: {
      default: "none",
      ":focus-visible": "none",
    },
    fontSize: "1rem",
    minHeight: "80px",
    paddingBlock: "0.5rem",
    paddingInline: 0,
    resize: "none",
  },
  memoLabel: {
    paddingTop: "0.5rem",
  },
  memoRow: {
    alignItems: "stretch",
  },
  mutedText: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  organizationControl: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
    maxWidth: "450px",
  },
  organizationForm: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  organizationHeading: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
  },
  organizationList: {
    borderColor: colors.border,
    borderRadius: radii.sm,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    flexDirection: "column",
    overflow: "hidden",
    width: "100%",
  },
  organizationListScrollable: {
    borderColor: colors.border,
    borderRadius: radii.sm,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    flexDirection: "column",
    maxHeight: "40vh",
    overflowX: "hidden",
    overflowY: "auto",
    width: "100%",
  },
  organizationName: {
    fontSize: "1rem",
    lineHeight: "1.5rem",
  },
  organizationOption: {
    alignItems: "center",
    display: "flex",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  organizationOptionIcon: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderRadius: radii.full,
    display: "flex",
    flexShrink: 0,
    height: "1.25rem",
    justifyContent: "center",
    marginRight: "0.5rem",
    width: "1.25rem",
  },
  organizationOptionName: {
    fontWeight: 500,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  organizationPanel: {
    padding: "0.75rem",
  },
  organizationSearch: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: radii.sm,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    gap: "0.5rem",
    paddingBlock: "0.375rem",
    paddingInline: "0.5rem",
    width: "100%",
  },
  organizationSearchIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
  },
  organizationSearchInput: {
    "::placeholder": {
      color: colors.mutedForeground,
    },
    backgroundColor: "transparent",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    outline: {
      default: null,
      ":focus": "none",
      ":focus-visible": "none",
    },
    width: "100%",
  },
  organizationTrigger: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.lg,
    cursor: "pointer",
    display: "inline-flex",
    marginInline: "-0.5rem",
    paddingBlock: "0.25rem",
    paddingInline: "0.5rem",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  partialSummaryError: {
    alignItems: "center",
    borderTopColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    display: "flex",
    gap: "0.75rem",
    justifyContent: "space-between",
    marginTop: "0.75rem",
    paddingTop: "0.75rem",
  },
  relaxedText: {
    lineHeight: 1.625,
  },
  removeOrganization: {
    color: {
      default: colors.mutedForeground,
      ":hover": "rgb(220 38 38)",
    },
    cursor: "pointer",
    height: "1rem",
    width: "1rem",
  },
  removeOrganizationContainer: {
    color: colors.mutedForeground,
    marginLeft: "0.5rem",
  },
  root: {
    display: "flex",
    flex: "1",
    flexDirection: "column",
    height: "100%",
  },
  scrollSpacer: {
    paddingBottom: "24rem",
  },
  scroller: {
    flex: "1",
    overflowY: "auto",
  },
  selectedOrganization: {
    alignItems: "center",
    display: "flex",
  },
  skeleton: {
    display: "flex",
    flexDirection: "column",
    gap: "0.625rem",
    paddingBlock: "0.25rem",
  },
  skeletonBullet: {
    animationDuration: "2s",
    animationIterationCount: "infinite",
    animationName: pulse,
    animationTimingFunction: "cubic-bezier(0.4, 0, 0.6, 1)",
    backgroundColor: `color-mix(in srgb, ${colors.mutedForeground} 20%, transparent)`,
    borderRadius: radii.full,
    flexShrink: 0,
    height: "0.25rem",
    width: "0.25rem",
  },
  skeletonLine: {
    animationDuration: "2s",
    animationIterationCount: "infinite",
    animationName: pulse,
    animationTimingFunction: "cubic-bezier(0.4, 0, 0.6, 1)",
    backgroundColor: `color-mix(in srgb, ${colors.mutedForeground} 10%, transparent)`,
    borderRadius: radii.full,
    height: "0.75rem",
  },
  skeletonRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.625rem",
  },
  skeletonWidth: (width: string) => ({
    width,
  }),
  smallIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  summaryCard: {
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    padding: "1rem",
  },
  summaryError: {
    alignItems: "center",
    display: "flex",
    gap: "0.75rem",
    justifyContent: "space-between",
  },
  summaryHeader: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    marginBottom: "0.75rem",
  },
  summaryHeading: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
  },
  summarySection: {
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    padding: "1.5rem",
  },
  summarySpinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    color: colors.mutedForeground,
    height: "0.875rem",
    width: "0.875rem",
  },
  unhighlightedOption: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
  },
  visuallyHidden: {
    borderWidth: 0,
    clipPath: "inset(50%)",
    height: "1px",
    margin: "-1px",
    overflow: "hidden",
    padding: 0,
    position: "absolute",
    whiteSpace: "nowrap",
    width: "1px",
  },
});
