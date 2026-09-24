import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery } from "@tanstack/react-query";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { CircleNotch, Sparkle } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";

import { crmProvidersQueryOptions } from "./connection";
import {
  type CrmEnrichmentField,
  type CrmEnrichmentResult,
  enrichHumanFromCrm,
} from "./enrichment";

import { useAuth } from "~/auth";
import { useConnections } from "~/auth/useConnections";
import { type HumanRecord } from "~/contacts/queries";
import { useTabs } from "~/store/zustand/tabs";

/**
 * Pulls job title, company, phone, LinkedIn and missing name/email for a
 * contact from the CRMs connected in Settings › CRM. Distinct from the
 * calendar-based "Enhance contact" action, which only reads event data.
 */
export function EnrichContactFromCrm({
  human,
  ownerUserId,
}: {
  human: HumanRecord;
  ownerUserId: string;
}) {
  const { t } = useLingui();
  const openNew = useTabs((state) => state.openNew);
  const auth = useAuth();
  const connections = useConnections(!!auth?.session);
  const providers = useQuery(crmProvidersQueryOptions());
  const enrich = useMutation({
    mutationKey: ["crm", "enrich", human.id],
    mutationFn: () =>
      enrichHumanFromCrm({
        human,
        ownerUserId,
        providers: providers.data ?? [],
        connections: connections.data ?? [],
        headers: auth?.getHeaders() ?? {},
      }),
  });

  if (providers.data && providers.data.length === 0) return null;

  const openCrmSettings = () =>
    openNew({ type: "settings", state: { tab: "crm" } });

  return (
    <div className="border-border flex flex-col gap-2 border-b px-4 py-3">
      <div className="flex items-center justify-between gap-3">
        <div className="text-muted-foreground text-sm">
          <Trans>CRM</Trans>
        </div>
        <Button
          size="sm"
          variant="outline"
          disabled={enrich.isPending || !providers.data}
          onClick={() => enrich.mutate()}
          aria-label={t`Enrich contact from CRM`}
        >
          {enrich.isPending ? (
            <CircleNotch className="size-4 animate-spin" />
          ) : (
            <Sparkle className="size-4" />
          )}
          <Trans>Enrich from CRM</Trans>
        </Button>
      </div>
      {enrich.isError && (
        <p role="alert" className="text-destructive text-xs">
          {enrich.error instanceof Error
            ? enrich.error.message
            : t`Could not reach the CRM`}
        </p>
      )}
      {enrich.data && (
        <EnrichmentOutcome
          result={enrich.data}
          onOpenSettings={openCrmSettings}
        />
      )}
    </div>
  );
}

function EnrichmentOutcome({
  result,
  onOpenSettings,
}: {
  result: CrmEnrichmentResult;
  onOpenSettings: () => void;
}) {
  const { t } = useLingui();

  if (result.status === "not_connected") {
    return (
      <p className="text-muted-foreground text-xs">
        <Trans>No CRM connected.</Trans>{" "}
        <button
          type="button"
          className="underline underline-offset-2"
          onClick={onOpenSettings}
        >
          <Trans>Connect one in Settings</Trans>
        </button>
      </p>
    );
  }

  if (result.status === "no_match") {
    const providers = result.providers.join(", ");
    return (
      <p className="text-muted-foreground text-xs">
        <Trans>No matching contact found in {providers}.</Trans>
      </p>
    );
  }

  const fields = Object.keys(result.changes) as CrmEnrichmentField[];
  const labels: Record<CrmEnrichmentField, string> = {
    name: t`name`,
    email: t`email`,
    companyName: t`company`,
    jobTitle: t`job title`,
    phone: t`phone`,
    linkedinUsername: t`LinkedIn`,
  };
  const provider = result.provider.name;
  const filled = fields.map((field) => labels[field]).join(", ");
  const url = result.contact.url;

  return (
    <p className="text-muted-foreground text-xs">
      {fields.length > 0 ? (
        <Trans>
          Filled {filled} from {provider}.
        </Trans>
      ) : (
        <Trans>Matched in {provider}; nothing new to add.</Trans>
      )}
      {url && (
        <>
          {" "}
          <button
            type="button"
            className="underline underline-offset-2"
            onClick={() => void openerCommands.openUrl(url, null)}
          >
            <Trans>Open record</Trans>
          </button>
        </>
      )}
    </p>
  );
}
