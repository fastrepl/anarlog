import { Icon } from "@iconify-icon/react";
import { Trans, useLingui } from "@lingui/react/macro";
import { Building2, Mail } from "lucide-react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { cn } from "@hypr/utils";

import {
  type HumanRecord,
  type OrganizationRecord,
  updateOrganization,
} from "./queries";
import { ContactFacehash, getContactBgClass } from "./shared";

export function OrganizationDetailsColumn({
  organization,
  humans,
  onPersonClick,
}: {
  organization: OrganizationRecord | null;
  humans: HumanRecord[];
  onPersonClick?: (personId: string) => void;
}) {
  const { t } = useLingui();
  const peopleInOrg = organization
    ? humans.filter((human) => human.organizationId === organization.id)
    : [];

  return (
    <div className="flex flex-1 flex-col">
      {organization ? (
        <>
          <div
            data-tauri-drag-region
            className="border-border flex items-center justify-center border-b py-6"
          >
            <div className="bg-accent flex h-16 w-16 items-center justify-center rounded-full">
              <Building2 className="text-muted-foreground h-8 w-8" />
            </div>
          </div>

          <div className="flex-1 overflow-y-auto">
            <div>
              <div className="border-border flex items-center border-b px-4 py-3">
                <div className="text-muted-foreground w-28 text-sm">
                  <Trans>Name</Trans>
                </div>
                <div className="flex-1">
                  <EditableOrganizationNameField
                    key={organization.id}
                    organization={organization}
                  />
                </div>
              </div>
            </div>

            <div className="p-6">
              <h3 className="text-muted-foreground mb-4 text-sm font-medium">
                <Trans>People</Trans>
                <span className="text-muted-foreground font-normal">
                  {" "}
                  &middot; {peopleInOrg.length}{" "}
                  {peopleInOrg.length === 1 ? t`member` : t`members`}
                </span>
              </h3>
              <div className="overflow-y-auto" style={{ maxHeight: "55vh" }}>
                {peopleInOrg.length > 0 ? (
                  <div className="grid grid-cols-3 gap-4">
                    {peopleInOrg.map((human) => {
                      return (
                        <div
                          key={human.id}
                          className="border-border bg-card cursor-pointer rounded-lg border p-4 transition-all hover:shadow-xs"
                          onClick={() => onPersonClick?.(human.id)}
                        >
                          <div className="flex flex-col items-center gap-3 text-center">
                            <div
                              className={cn([
                                "shrink-0 rounded-full",
                                getContactBgClass(
                                  String(human.name || human.email || human.id),
                                ),
                              ])}
                            >
                              <ContactFacehash
                                name={String(
                                  human.name || human.email || human.id,
                                )}
                                size={48}
                                interactive={false}
                                showInitial={false}
                                colorClasses={[
                                  getContactBgClass(
                                    String(
                                      human.name || human.email || human.id,
                                    ),
                                  ),
                                ]}
                              />
                            </div>
                            <div className="w-full">
                              <div className="truncate text-sm font-semibold">
                                {human.name || human.email || t`Unnamed`}
                              </div>
                              {human.jobTitle && (
                                <div className="text-muted-foreground mt-1 truncate text-xs">
                                  {human.jobTitle}
                                </div>
                              )}
                            </div>
                            <div className="mt-1 flex gap-2">
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
                                  <Mail />
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
                  <p className="text-muted-foreground text-sm">
                    <Trans>No people in this organization</Trans>
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
      className="h-7 border-none p-0 text-base shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
    />
  );
}
