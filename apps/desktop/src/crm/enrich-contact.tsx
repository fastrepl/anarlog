import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { CircleNotch, Sparkle } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@anlg/ui/components/ui/popover";

import { crmProvidersQueryOptions } from "./connection";
import {
  connectedCrmProviders,
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
export function useCrmEnrichment({
  ownerUserId,
}: {
  human: HumanRecord | null;
  ownerUserId: string;
}) {
  const auth = useAuth();
  const connections = useConnections(!!auth?.session);
  const providers = useQuery(crmProvidersQueryOptions());
  const mutation = useMutation({
    mutationKey: ["crm", "enrich"],
    mutationFn: (target: HumanRecord) =>
      enrichHumanFromCrm({
        human: target,
        ownerUserId,
        providers: providers.data ?? [],
        connections: connections.data ?? [],
        headers: auth?.getHeaders() ?? {},
      }),
  });

  const signedIn = !!auth?.session;
  const connected = connectedCrmProviders(
    providers.data ?? [],
    connections.data,
  );

  return {
    available: !(providers.data && providers.data.length === 0),
    ready: !!providers.data,
    connected: connected.length > 0,
    loading: !providers.data || (signedIn && connections.isPending),
    mutation,
  };
}

export type CrmEnrichment = ReturnType<typeof useCrmEnrichment>;

export function EnrichContactButton({
  enrichment,
  human,
}: {
  enrichment: CrmEnrichment;
  human: HumanRecord;
}) {
  const { t } = useLingui();
  const { available, ready, connected, loading, mutation } = enrichment;
  const [promptOpen, setPromptOpen] = useState(false);
  const pendingForThisHuman =
    mutation.isPending && mutation.variables?.id === human.id;
  const showConnectPrompt = !loading && !connected;

  if (!available) return null;

  return (
    <Popover open={promptOpen && showConnectPrompt}>
      <PopoverAnchor asChild>
        <span
          className="inline-flex"
          tabIndex={showConnectPrompt ? 0 : undefined}
          onPointerEnter={() => setPromptOpen(true)}
          onPointerLeave={() => setPromptOpen(false)}
          onFocus={() => setPromptOpen(true)}
          onBlur={() => setPromptOpen(false)}
        >
          <Button
            size="sm"
            variant="ghost"
            className="text-muted-foreground hover:text-foreground rounded-full"
            data-tauri-drag-region="false"
            disabled={!connected || loading || pendingForThisHuman || !ready}
            onClick={() => mutation.mutate(human)}
            aria-label={t`Enrich contact from CRM`}
          >
            {pendingForThisHuman ? (
              <CircleNotch className="size-4 animate-spin" />
            ) : (
              <Sparkle className="size-4" />
            )}
            <Trans>Enrich</Trans>
          </Button>
        </span>
      </PopoverAnchor>
      <PopoverContent
        data-crm-enrich-prompt
        side="bottom"
        align="end"
        sideOffset={10}
        onOpenAutoFocus={(event) => event.preventDefault()}
        className="border-border bg-popover text-popover-foreground pointer-events-none w-72 max-w-[calc(100vw-1rem)] rounded-md border px-3 py-2.5 text-sm shadow-sm"
      >
        <span
          aria-hidden="true"
          className="border-border bg-popover absolute -top-1.5 right-4 size-3 rotate-45 border-t border-l"
        />
        <span className="relative block font-medium">{t`Connect a CRM`}</span>
        <span className="text-muted-foreground relative mt-0.5 block leading-snug">
          {t`Connect a CRM in Settings › CRM to enrich contacts.`}
        </span>
      </PopoverContent>
    </Popover>
  );
}

export function EnrichContactStatus({
  enrichment,
  human,
}: {
  enrichment: CrmEnrichment;
  human: HumanRecord;
}) {
  const { t } = useLingui();
  const openNew = useTabs((state) => state.openNew);
  const { mutation } = enrichment;
  const forThisHuman = mutation.variables?.id === human.id;
  const showError = mutation.isError && forThisHuman;
  const result = mutation.data && forThisHuman ? mutation.data : undefined;

  if (!showError && !result) return null;

  const openCrmSettings = () =>
    openNew({ type: "settings", state: { tab: "crm" } });

  return (
    <div className="border-border border-b px-4 py-3">
      {showError && (
        <p role="alert" className="text-destructive text-xs">
          {mutation.error instanceof Error
            ? mutation.error.message
            : t`Could not reach the CRM`}
        </p>
      )}
      {result && (
        <EnrichmentOutcome result={result} onOpenSettings={openCrmSettings} />
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
