import { Icon } from "@iconify-icon/react";
import { Trans, useLingui } from "@lingui/react/macro";
import { Buildings, Envelope } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useState } from "react";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";

import {
  AvatarUploadButton,
  ContactImage,
  persistContactAvatar,
} from "./contact-avatar";
import { ContactPageHeader } from "./contact-page-header";
import {
  type HumanRecord,
  type OrganizationRecord,
  toggleContactPin,
  updateOrganization,
} from "./queries";
import { ContactFacehash } from "./shared";

export function OrganizationDetailsColumn({
  organization,
  humans,
  onPersonClick,
  onDelete,
}: {
  organization: OrganizationRecord | null;
  humans: HumanRecord[];
  onPersonClick?: (personId: string) => void;
  onDelete: (id: string) => void;
}) {
  const { t } = useLingui();
  const [showCompactIdentity, setShowCompactIdentity] = useState(false);
  const peopleInOrg = organization
    ? humans.filter((human) => human.organizationId === organization.id)
    : [];

  return (
    <div {...stylex.props(styles.root)}>
      {organization ? (
        <>
          <ContactPageHeader
            title={organization.name || t`Unnamed`}
            compactIdentity={
              organization.avatarDataUrl ? (
                <ContactImage src={organization.avatarDataUrl} size={24} />
              ) : (
                <div {...stylex.props(styles.compactAvatar)}>
                  <Buildings {...stylex.props(styles.compactAvatarIcon)} />
                </div>
              )
            }
            showCompactIdentity={showCompactIdentity}
            pinned={Boolean(organization.pinned)}
            onTogglePin={() => {
              void toggleContactPin("organization", organization.id).catch(
                (error) => {
                  console.error(
                    "[contacts] failed to toggle contact pin",
                    error,
                  );
                },
              );
            }}
            onDelete={() => onDelete(organization.id)}
            onRemoveAvatar={
              organization.avatarDataUrl
                ? () =>
                    persistContactAvatar("organization", organization.id, null)
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
                  persistContactAvatar("organization", organization.id, dataUrl)
                }
              >
                {organization.avatarDataUrl ? (
                  <ContactImage src={organization.avatarDataUrl} size={64} />
                ) : (
                  <div {...stylex.props(styles.largeAvatar)}>
                    <Buildings {...stylex.props(styles.largeAvatarIcon)} />
                  </div>
                )}
              </AvatarUploadButton>
            </div>

            <div>
              <div {...stylex.props(styles.fieldRow)}>
                <div {...stylex.props(styles.fieldLabel)}>
                  <Trans>Name</Trans>
                </div>
                <div {...stylex.props(styles.fieldControl)}>
                  <EditableOrganizationNameField
                    key={organization.id}
                    organization={organization}
                  />
                </div>
              </div>
            </div>

            <div {...stylex.props(styles.peopleSection)}>
              <h3 {...stylex.props(styles.peopleHeading)}>
                <Trans>People</Trans>
                <span {...stylex.props(styles.peopleCount)}>
                  {" "}
                  &middot; {peopleInOrg.length}{" "}
                  {peopleInOrg.length === 1 ? t`member` : t`members`}
                </span>
              </h3>
              <div>
                {peopleInOrg.length > 0 ? (
                  <div {...stylex.props(styles.peopleGrid)}>
                    {peopleInOrg.map((human) => {
                      return (
                        <div
                          key={human.id}
                          {...stylex.props(styles.personCard)}
                          onClick={() => onPersonClick?.(human.id)}
                        >
                          <div {...stylex.props(styles.personCardContent)}>
                            {human.avatarDataUrl ? (
                              <ContactImage
                                src={human.avatarDataUrl}
                                size={48}
                              />
                            ) : (
                              <ContactFacehash
                                name={String(
                                  human.name || human.email || human.id,
                                )}
                                size={48}
                              />
                            )}
                            <div {...stylex.props(styles.personCopy)}>
                              <div {...stylex.props(styles.personName)}>
                                {human.name || human.email || t`Unnamed`}
                              </div>
                              {human.jobTitle && (
                                <div {...stylex.props(styles.jobTitle)}>
                                  {human.jobTitle}
                                </div>
                              )}
                            </div>
                            <div {...stylex.props(styles.personActions)}>
                              {human.email && (
                                <Button
                                  variant="ghost"
                                  size="icon"
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    void openerCommands.openUrl(
                                      `mailto:${human.email}`,
                                      null,
                                    );
                                  }}
                                  title={t`Send email`}
                                >
                                  <Envelope
                                    {...stylex.props(styles.actionIcon)}
                                  />
                                </Button>
                              )}
                              {human.linkedinUsername && (
                                <Button
                                  variant="ghost"
                                  size="icon"
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    const v = String(human.linkedinUsername);
                                    const href = /^https?:\/\//i.test(v)
                                      ? v
                                      : `https://www.linkedin.com/in/${v.replace(/^@/, "")}`;
                                    void openerCommands.openUrl(href, null);
                                  }}
                                  title={t`View LinkedIn profile`}
                                >
                                  <Icon icon="logos:linkedin-icon" />
                                </Button>
                              )}
                            </div>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                ) : (
                  <p {...stylex.props(styles.emptyText)}>
                    <Trans>No people in this organization</Trans>
                  </p>
                )}
              </div>
            </div>

            <div {...stylex.props(styles.scrollSpacer)} />
          </div>
        </>
      ) : (
        <div {...stylex.props(styles.emptyState)}>
          <p {...stylex.props(styles.emptyText)}>
            <Trans>Select an organization to view details</Trans>
          </p>
        </div>
      )}
    </div>
  );
}

function EditableOrganizationNameField({
  organization,
}: {
  organization: OrganizationRecord;
}) {
  const { t } = useLingui();

  return (
    <Input
      defaultValue={organization.name}
      onChange={(event) => {
        void updateOrganization(organization.id, {
          name: event.target.value,
        }).catch((error) => {
          console.error("[contacts] failed to update organization", error);
        });
      }}
      placeholder={t`Organization name`}
      sx={styles.nameInput}
    />
  );
}

const styles = stylex.create({
  actionIcon: {
    height: "1rem",
    width: "1rem",
  },
  avatarSection: {
    alignItems: "center",
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    display: "flex",
    justifyContent: "center",
    paddingBlock: "1.5rem",
  },
  compactAvatar: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderRadius: radii.full,
    display: "flex",
    flexShrink: 0,
    height: "1.5rem",
    justifyContent: "center",
    width: "1.5rem",
  },
  compactAvatarIcon: {
    color: colors.mutedForeground,
    height: "0.75rem",
    width: "0.75rem",
  },
  emptyState: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    justifyContent: "center",
  },
  emptyText: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  fieldControl: {
    flex: "1",
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
  jobTitle: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
    marginTop: "0.25rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  largeAvatar: {
    alignItems: "center",
    backgroundColor: colors.accent,
    borderRadius: radii.full,
    display: "flex",
    height: "4rem",
    justifyContent: "center",
    width: "4rem",
  },
  largeAvatarIcon: {
    color: colors.mutedForeground,
    height: "2rem",
    width: "2rem",
  },
  nameInput: {
    borderWidth: 0,
    boxShadow: {
      default: "none",
      ":focus-visible": "none",
    },
    fontSize: "1rem",
    height: "1.75rem",
    padding: 0,
  },
  peopleCount: {
    color: colors.mutedForeground,
    fontWeight: 400,
  },
  peopleGrid: {
    display: "grid",
    gap: "1rem",
    gridTemplateColumns: "repeat(3, minmax(0, 1fr))",
  },
  peopleHeading: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
    marginBottom: "1rem",
  },
  peopleSection: {
    padding: "1.5rem",
  },
  personActions: {
    display: "flex",
    gap: "0.5rem",
    marginTop: "0.25rem",
  },
  personCard: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: "none",
      ":hover": shadows.sm,
    },
    cursor: "pointer",
    padding: "1rem",
    transitionDuration: "150ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  personCardContent: {
    alignItems: "center",
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
    textAlign: "center",
  },
  personCopy: {
    width: "100%",
  },
  personName: {
    fontSize: "0.875rem",
    fontWeight: 600,
    lineHeight: "1.25rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
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
});
