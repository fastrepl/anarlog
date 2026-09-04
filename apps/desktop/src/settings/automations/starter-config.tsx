import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery } from "@tanstack/react-query";
import { open as selectFolder } from "@tauri-apps/plugin-dialog";
import { type ReactNode, useState } from "react";

import {
  type ConnectionItem,
  linearListTeams,
  notionSearchPages,
} from "@anlg/api-client";
import { createClient } from "@anlg/api-client/client";
import {
  CircleNotch,
  FolderOpen,
  LockSimple,
  MagnifyingGlass,
} from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn, formatDistanceToNow } from "@anlg/utils";

import { useAuth } from "~/auth";
import { useConnections } from "~/auth/useConnections";
import {
  type AutomationRunRecord,
  type AutomationTargetRef,
  parseAutomationRunRecord,
  parseAutomationTargetRef,
} from "~/automations/engine";
import { env } from "~/env";
import { listSlackChannels } from "~/session-sharing/delivery-client";
import { setSettingValue, useStoredSettingValue } from "~/settings/queries";
import { type SettingKey } from "~/settings/schema";
import { useOpenIntegrationUrl } from "~/shared/integration";

type TargetSettingKey =
  | "automation_slack_recap_channel"
  | "automation_linear_issues_team"
  | "automation_notion_update_page";

export function AutomationLastRunLine({
  settingKey,
  lastRun: lastRunOverride,
}: {
  settingKey?: SettingKey;
  lastRun?: AutomationRunRecord | null;
}) {
  const storedLastRun = parseAutomationRunRecord(
    useStoredSettingValue(settingKey ?? "automation_draft_template").value as
      | string
      | undefined,
  );
  const lastRun =
    lastRunOverride !== undefined ? lastRunOverride : storedLastRun;
  if (!lastRun) {
    return null;
  }
  const relative = formatDistanceToNow(new Date(lastRun.at), {
    addSuffix: true,
  });
  return (
    <p
      className={cn([
        "mt-3 truncate text-xs",
        lastRun.status === "error"
          ? "text-destructive"
          : "text-muted-foreground",
      ])}
      title={lastRun.detail}
    >
      {lastRun.status === "success" ? (
        <Trans>
          Last run {relative}: {lastRun.detail}
        </Trans>
      ) : (
        <Trans>
          Last run failed {relative}: {lastRun.detail}
        </Trans>
      )}
    </p>
  );
}

function IntegrationGate({
  integrationId,
  connectLabel,
  reconnectLabel,
  children,
}: {
  integrationId: string;
  connectLabel: ReactNode;
  reconnectLabel: ReactNode;
  children: (connection: ConnectionItem) => ReactNode;
}) {
  const connections = useConnections(true);
  const { openIntegration, openingAction } = useOpenIntegrationUrl();
  const connection = connections.data?.find(
    (candidate) => candidate.integration_id === integrationId,
  );

  if (connections.isLoading) {
    return (
      <p className="text-muted-foreground text-xs">
        <Trans>Checking connection…</Trans>
      </p>
    );
  }
  if (!connection || connection.status === "reconnect_required") {
    const reconnect = connection?.status === "reconnect_required";
    return (
      <Button
        type="button"
        size="sm"
        variant="outline"
        disabled={openingAction !== null}
        onClick={() =>
          openIntegration({
            nangoIntegrationId: integrationId,
            connectionId: connection?.connection_id,
            action: reconnect ? "reconnect" : "connect",
          })
        }
      >
        {openingAction ? (
          <CircleNotch className="size-3.5 animate-spin" aria-hidden="true" />
        ) : null}
        {reconnect ? reconnectLabel : connectLabel}
      </Button>
    );
  }
  return <>{children(connection)}</>;
}

function useSaveTarget(settingKey: TargetSettingKey) {
  const { t } = useLingui();
  return useMutation({
    mutationKey: ["automation-target", settingKey],
    mutationFn: (target: AutomationTargetRef) =>
      setSettingValue(settingKey, JSON.stringify(target)),
    onError: () => sonnerToast.error(t`Could not save the automation setting`),
  });
}

function useAuthedApiClient() {
  const auth = useAuth();
  const headers = auth.getHeaders();
  if (!headers) {
    return null;
  }
  return createClient({ baseUrl: env.VITE_API_URL, headers });
}

function ConfigRow({
  title,
  value,
  children,
}: {
  title: ReactNode;
  value: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div className="min-w-0">
        <h4 className="text-xs font-semibold">{title}</h4>
        <p className="text-muted-foreground mt-1 truncate text-xs">{value}</p>
      </div>
      {children}
    </div>
  );
}

export function MarkdownExportConfig({
  value,
  onChange,
}: {
  value?: string;
  onChange?: (directory: string) => void;
} = {}) {
  const { t } = useLingui();
  const storedDirectory = (
    useStoredSettingValue("automation_markdown_export_directory").value ?? ""
  ).trim();
  const directory = (value ?? storedDirectory).trim();
  const chooseFolderMutation = useMutation({
    mutationKey: ["automation-markdown-export-folder"],
    mutationFn: async () => {
      const selected = await selectFolder({
        title: t`Choose export folder`,
        directory: true,
        multiple: false,
        defaultPath: directory || undefined,
      });
      if (typeof selected === "string" && selected) {
        if (onChange) {
          onChange(selected);
          return;
        }
        await setSettingValue("automation_markdown_export_directory", selected);
      }
    },
    onError: () => sonnerToast.error(t`Could not update the export folder`),
  });

  return (
    <ConfigRow
      title={<Trans>Export folder</Trans>}
      value={directory || <Trans>No folder selected yet.</Trans>}
    >
      <Button
        type="button"
        size="sm"
        variant="outline"
        onClick={() => chooseFolderMutation.mutate()}
        disabled={chooseFolderMutation.isPending}
      >
        <FolderOpen size={14} />
        <Trans>Choose folder</Trans>
      </Button>
    </ConfigRow>
  );
}

export function SlackRecapConfig({
  value,
  onChange,
}: {
  value?: AutomationTargetRef | null;
  onChange?: (target: AutomationTargetRef) => void;
} = {}) {
  const selected =
    value !== undefined
      ? value
      : parseAutomationTargetRef(
          useStoredSettingValue("automation_slack_recap_channel").value,
        );

  return (
    <ConfigRow
      title={<Trans>Slack channel</Trans>}
      value={
        selected ? `#${selected.name}` : <Trans>No channel selected yet.</Trans>
      }
    >
      <IntegrationGate
        integrationId="slack"
        connectLabel={<Trans>Connect Slack</Trans>}
        reconnectLabel={<Trans>Reconnect Slack</Trans>}
      >
        {() => <SlackChannelSelect selected={selected} onChange={onChange} />}
      </IntegrationGate>
    </ConfigRow>
  );
}

function SlackChannelSelect({
  selected,
  onChange,
}: {
  selected: AutomationTargetRef | null;
  onChange?: (target: AutomationTargetRef) => void;
}) {
  const { t } = useLingui();
  const auth = useAuth();
  const saveTarget = useSaveTarget("automation_slack_recap_channel");
  const applyTarget = (target: AutomationTargetRef) => {
    if (onChange) {
      onChange(target);
      return;
    }
    saveTarget.mutate(target);
  };
  const channels = useQuery({
    queryKey: ["automation-slack-channels", auth.session?.user.id],
    enabled: Boolean(auth.session?.access_token),
    queryFn: ({ signal }) =>
      listSlackChannels({
        apiBaseUrl: env.VITE_API_URL,
        accessToken: auth.session?.access_token ?? "",
        signal,
      }),
  });

  return (
    <Select
      value={selected?.id ?? ""}
      onValueChange={(id) => {
        const channel = channels.data?.find((entry) => entry.id === id);
        if (channel) {
          applyTarget({ id: channel.id, name: channel.name });
        }
      }}
      disabled={channels.isLoading || (!onChange && saveTarget.isPending)}
    >
      <SelectTrigger className="h-8 w-52 text-xs">
        <SelectValue
          placeholder={
            channels.isLoading ? t`Loading channels…` : t`Choose a channel`
          }
        />
      </SelectTrigger>
      <SelectContent>
        {channels.data?.map((channel) => (
          <SelectItem key={channel.id} value={channel.id}>
            <span className="flex items-center gap-1.5">
              {channel.isPrivate ? (
                <LockSimple className="size-3" aria-hidden="true" />
              ) : (
                <span aria-hidden="true">#</span>
              )}
              {channel.name}
            </span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

export function LinearIssuesConfig({
  value,
  onChange,
}: {
  value?: AutomationTargetRef | null;
  onChange?: (target: AutomationTargetRef) => void;
} = {}) {
  const selected =
    value !== undefined
      ? value
      : parseAutomationTargetRef(
          useStoredSettingValue("automation_linear_issues_team").value,
        );

  return (
    <ConfigRow
      title={<Trans>Linear team</Trans>}
      value={selected?.name ?? <Trans>No team selected yet.</Trans>}
    >
      <IntegrationGate
        integrationId="linear"
        connectLabel={<Trans>Connect Linear</Trans>}
        reconnectLabel={<Trans>Reconnect Linear</Trans>}
      >
        {(connection) => (
          <LinearTeamSelect
            connection={connection}
            selected={selected}
            onChange={onChange}
          />
        )}
      </IntegrationGate>
    </ConfigRow>
  );
}

function LinearTeamSelect({
  connection,
  selected,
  onChange,
}: {
  connection: ConnectionItem;
  selected: AutomationTargetRef | null;
  onChange?: (target: AutomationTargetRef) => void;
}) {
  const { t } = useLingui();
  const client = useAuthedApiClient();
  const saveTarget = useSaveTarget("automation_linear_issues_team");
  const applyTarget = (target: AutomationTargetRef) => {
    if (onChange) {
      onChange(target);
      return;
    }
    saveTarget.mutate(target);
  };
  const teams = useQuery({
    queryKey: ["automation-linear-teams", connection.connection_id],
    enabled: client !== null,
    queryFn: async () => {
      if (!client) {
        throw new Error("not signed in");
      }
      const { data, error } = await linearListTeams({
        client,
        body: { connection_id: connection.connection_id },
      });
      if (error) {
        throw new Error("Failed to load Linear teams");
      }
      return data?.items ?? [];
    },
  });

  return (
    <Select
      value={selected?.id ?? ""}
      onValueChange={(id) => {
        const team = teams.data?.find((entry) => entry.id === id);
        if (team) {
          applyTarget({ id: team.id, name: team.name });
        }
      }}
      disabled={teams.isLoading || (!onChange && saveTarget.isPending)}
    >
      <SelectTrigger className="h-8 w-52 text-xs">
        <SelectValue
          placeholder={teams.isLoading ? t`Loading teams…` : t`Choose a team`}
        />
      </SelectTrigger>
      <SelectContent>
        {teams.data?.map((team) => (
          <SelectItem key={team.id} value={team.id}>
            {team.name}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

export function NotionUpdateConfig({
  value,
  onChange,
}: {
  value?: AutomationTargetRef | null;
  onChange?: (target: AutomationTargetRef) => void;
} = {}) {
  const selected =
    value !== undefined
      ? value
      : parseAutomationTargetRef(
          useStoredSettingValue("automation_notion_update_page").value,
        );

  return (
    <div className="flex flex-col gap-3">
      <ConfigRow
        title={<Trans>Notion page</Trans>}
        value={selected?.name ?? <Trans>No page selected yet.</Trans>}
      />
      <IntegrationGate
        integrationId="notion"
        connectLabel={<Trans>Connect Notion</Trans>}
        reconnectLabel={<Trans>Reconnect Notion</Trans>}
      >
        {(connection) => (
          <NotionPageSearch
            connection={connection}
            selected={selected}
            onChange={onChange}
          />
        )}
      </IntegrationGate>
    </div>
  );
}

function NotionPageSearch({
  connection,
  selected,
  onChange,
}: {
  connection: ConnectionItem;
  selected: AutomationTargetRef | null;
  onChange?: (target: AutomationTargetRef) => void;
}) {
  const { t } = useLingui();
  const client = useAuthedApiClient();
  const saveTarget = useSaveTarget("automation_notion_update_page");
  const applyTarget = (target: AutomationTargetRef) => {
    if (onChange) {
      onChange(target);
      return;
    }
    saveTarget.mutate(target);
  };
  const [query, setQuery] = useState("");
  const [submittedQuery, setSubmittedQuery] = useState<string | null>(null);
  const pages = useQuery({
    queryKey: [
      "automation-notion-pages",
      connection.connection_id,
      submittedQuery,
    ],
    enabled: submittedQuery !== null && client !== null,
    queryFn: async () => {
      if (!client) {
        throw new Error("not signed in");
      }
      const { data, error } = await notionSearchPages({
        client,
        body: {
          connection_id: connection.connection_id,
          query: submittedQuery || null,
        },
      });
      if (error) {
        throw new Error("Failed to search Notion pages");
      }
      return data?.pages ?? [];
    },
  });

  return (
    <div className="flex flex-col gap-2">
      <div className="flex gap-2">
        <input
          type="text"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              setSubmittedQuery(query.trim());
            }
          }}
          placeholder={t`Search pages shared with Anarlog…`}
          className={cn([
            "border-border bg-accent/50 h-8 min-w-0 flex-1 rounded-lg border px-3 text-xs",
            "placeholder:text-muted-foreground focus:outline-hidden",
          ])}
        />
        <Button
          type="button"
          size="sm"
          variant="outline"
          onClick={() => setSubmittedQuery(query.trim())}
          disabled={pages.isFetching}
        >
          {pages.isFetching ? (
            <CircleNotch className="size-3.5 animate-spin" aria-hidden="true" />
          ) : (
            <MagnifyingGlass size={14} />
          )}
          <Trans>Search</Trans>
        </Button>
      </div>
      {pages.data && pages.data.length > 0 ? (
        <div className="flex flex-col gap-1">
          {pages.data.map((page) => (
            <button
              key={page.id}
              type="button"
              onClick={() => applyTarget({ id: page.id, name: page.title })}
              className={cn([
                "rounded-lg px-3 py-1.5 text-left text-xs transition-colors",
                page.id === selected?.id ? "bg-accent" : "hover:bg-accent/50",
              ])}
            >
              {page.title}
            </button>
          ))}
        </div>
      ) : pages.data ? (
        <p className="text-muted-foreground text-xs">
          <Trans>
            No pages found. Share the page with the Anarlog integration in
            Notion first.
          </Trans>
        </p>
      ) : null}
    </div>
  );
}
