import { Icon } from "@iconify-icon/react";
import { Facehash } from "facehash";
import { Building2, Mail } from "lucide-react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { cn } from "@hypr/utils";

import { getContactBgClass } from "./shared";

import * as main from "~/store/tinybase/store/main";

export function OrganizationDetailsColumn({
  selectedOrganizationId,
  onPersonClick,
}: {
  selectedOrganizationId?: string | null;
  onPersonClick?: (personId: string) => void;
}) {
  const selectedOrgData = main.UI.useRow(
    "organizations",
    selectedOrganizationId ?? "",
    main.STORE_ID,
  );

  const peopleInOrg = main.UI.useSliceRowIds(
    main.INDEXES.humansByOrg,
    selectedOrganizationId ?? "",
    main.STORE_ID,
  );

  const allHumans = main.UI.useTable("humans", main.STORE_ID);

  return (
    <div className="flex flex-1 flex-col">
      {selectedOrgData && selectedOrganizationId ? (
        <>
          <div className="flex items-center justify-center border-b border-neutral-200 py-6">
            <div className="flex h-16 w-16 items-center justify-center rounded-full bg-neutral-200">
              <Building2 className="h-8 w-8 text-neutral-600" />
            </div>
          </div>

          <div className="flex-1 overflow-y-auto">
            <div>
              <div className="flex items-center border-b border-neutral-200 px-4 py-3">
                <div className="w-28 text-sm text-neutral-500">Name</div>
                <div className="flex-1">
                  <EditableOrganizationNameField
                    organizationId={selectedOrganizationId}
                  />
                </div>
              </div>
            </div>

            <div className="p-6">
              <h3 className="mb-4 text-sm font-medium text-neutral-600">
                People
                <span className="font-normal text-neutral-400">
                  {" "}
                  &middot; {peopleInOrg?.length ?? 0}{" "}
                  {(peopleInOrg?.length ?? 0) === 1 ? "member" : "members"}
                </span>
              </h3>
              <div className="overflow-y-auto" style={{ maxHeight: "55vh" }}>
                {(peopleInOrg?.length ?? 0) > 0 ? (
                  <div className="grid grid-cols-3 gap-4">
                    {peopleInOrg.map((humanId: string) => {
                      const human = allHumans[humanId];
                      if (!human) {
                        return null;
                      }

                      return (
                        <div
                          key={humanId}
                          className="cursor-pointer rounded-lg border border-neutral-200 bg-white p-4 transition-all hover:shadow-xs"
                          onClick={() => onPersonClick?.(humanId)}
                        >
                          <div className="flex flex-col items-center gap-3 text-center">
                            <div
                              className={cn([
                                "shrink-0 rounded-full",
                                getContactBgClass(
                                  String(human.name || human.email || humanId),
                                ),
                              ])}
                            >
                              <Facehash
                                name={String(
                                  human.name || human.email || humanId,
                                )}
                                size={48}
                                interactive={false}
                                showInitial={false}
                                colorClasses={[
                                  getContactBgClass(
                                    String(
                                      human.name || human.email || humanId,
                                    ),
                                  ),
                                ]}
                              />
                            </div>
                            <div className="w-full">
                              <div className="truncate text-sm font-semibold">
                                {human.name || human.email || "Unnamed"}
                              </div>
                              {human.job_title && (
                                <div className="mt-1 truncate text-xs text-neutral-500">
                                  {human.job_title as string}
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
                                  title="Send email"
                                >
                                  <Mail />
                                </Button>
                              )}
                              {human.linkedin_username && (
                                <Button
                                  variant="ghost"
                                  size="icon"
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    const v = String(
                                      human.linkedin_username ?? "",
                                    );
                                    const href = /^https?:\/\//i.test(v)
                                      ? v
                                      : `https://www.linkedin.com/in/${v.replace(/^@/, "")}`;
                                    void openerCommands.openUrl(href, null);
                                  }}
                                  title="View LinkedIn profile"
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
                  <p className="text-sm text-neutral-500">
                    No people in this organization
                  </p>
                )}
              </div>
            </div>

            <div className="pb-96" />
          </div>
        </>
      ) : (
        <div className="flex flex-1 items-center justify-center">
          <p className="text-sm text-neutral-500">
            Select an organization to view details
          </p>
        </div>
      )}
    </div>
  );
}

function EditableOrganizationNameField({
  organizationId,
}: {
  organizationId: string;
}) {
  const value = main.UI.useCell(
    "organizations",
    organizationId,
    "name",
    main.STORE_ID,
  );

  const handleChange = main.UI.useSetCellCallback(
    "organizations",
    organizationId,
    "name",
    (e: React.ChangeEvent<HTMLInputElement>) => e.target.value,
    [],
    main.STORE_ID,
  );

  return (
    <Input
      value={(value as string) || ""}
      onChange={handleChange}
      placeholder="Organization name"
      className="h-7 border-none p-0 text-base shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
    />
  );
}
