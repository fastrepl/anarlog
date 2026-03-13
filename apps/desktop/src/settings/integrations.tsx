import { Icon } from "@iconify-icon/react";
import { Puzzle, Search } from "lucide-react";
import { useMemo, useState } from "react";

import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";
import { ButtonGroup } from "@hypr/ui/components/ui/button-group";
import { Input } from "@hypr/ui/components/ui/input";
import { Spinner } from "@hypr/ui/components/ui/spinner";

import { ConnectedServiceCard } from "./shared";

type IntegrationProvider =
  | "slack"
  | "notion"
  | "discord"
  | "linear"
  | "github"
  | "jira";

interface Integration {
  id: string;
  provider: IntegrationProvider;
  name: string;
  description: string;
  connected: boolean;
  connectedAt?: string;
  accountInfo?: string;
}

const MOCK_INTEGRATIONS: Integration[] = [
  {
    id: "1",
    provider: "slack",
    name: "Slack",
    description: "Send meeting notes and updates to Slack channels",
    connected: true,
    connectedAt: "2024-10-10T09:00:00Z",
    accountInfo: "Char Workspace",
  },
  {
    id: "2",
    provider: "notion",
    name: "Notion",
    description: "Sync your notes and transcripts with Notion databases",
    connected: true,
    connectedAt: "2024-09-15T14:30:00Z",
    accountInfo: "john@char.com",
  },
  {
    id: "3",
    provider: "discord",
    name: "Discord",
    description: "Post meeting summaries to Discord channels",
    connected: false,
  },
  {
    id: "4",
    provider: "linear",
    name: "Linear",
    description: "Create issues and sync action items with Linear",
    connected: false,
  },
  {
    id: "5",
    provider: "github",
    name: "GitHub",
    description: "Create issues and link discussions to repositories",
    connected: false,
  },
  {
    id: "6",
    provider: "jira",
    name: "Jira",
    description: "Create and update Jira tickets from meeting notes",
    connected: false,
  },
];

function getProviderIcon(provider: IntegrationProvider) {
  const iconMap: Record<IntegrationProvider, string> = {
    slack: "logos:slack-icon",
    notion: "logos:notion-icon",
    discord: "logos:discord-icon",
    linear: "logos:linear-icon",
    github: "logos:github-icon",
    jira: "logos:jira",
  };

  return <Icon icon={iconMap[provider]} size={20} />;
}

type FilterStatus = "all" | "connected" | "not-connected";

export function SettingsIntegrations() {
  const [searchQuery, setSearchQuery] = useState("");
  const [filterStatus, setFilterStatus] = useState<FilterStatus>("all");
  const integrations = MOCK_INTEGRATIONS;

  const filteredIntegrations = useMemo(() => {
    let filtered = integrations;

    // Apply status filter
    if (filterStatus === "connected") {
      filtered = filtered.filter((integration) => integration.connected);
    } else if (filterStatus === "not-connected") {
      filtered = filtered.filter((integration) => !integration.connected);
    }

    // Apply search filter
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter(
        (integration) =>
          integration.name.toLowerCase().includes(query) ||
          integration.description.toLowerCase().includes(query),
      );
    }

    return filtered;
  }, [searchQuery, filterStatus, integrations]);

  const handleConnect = (integrationId: string) => {
    // TODO: Implement connect logic
    console.log("Connect integration:", integrationId);
  };

  const handleDisconnect = (integrationId: string) => {
    // TODO: Implement disconnect logic
    console.log("Disconnect integration:", integrationId);
  };

  return (
    <div className="flex flex-col gap-8">
      <div>
        <div className="mb-4 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <h2 className="cursor-default font-serif font-semibold">
              Integrations
            </h2>
            <Badge variant="secondary" className="text-xs">
              Coming soon
            </Badge>
          </div>
          <ButtonGroup>
            <Button
              variant={filterStatus === "all" ? "default" : "outline"}
              size="sm"
              onClick={() => setFilterStatus("all")}
              className="shadow-none"
            >
              All
            </Button>
            <Button
              variant={filterStatus === "connected" ? "default" : "outline"}
              size="sm"
              onClick={() => setFilterStatus("connected")}
              className="shadow-none"
            >
              Connected
            </Button>
            <Button
              variant={filterStatus === "not-connected" ? "default" : "outline"}
              size="sm"
              onClick={() => setFilterStatus("not-connected")}
              className="shadow-none"
            >
              Not Connected
            </Button>
          </ButtonGroup>
        </div>

        <div className="relative mb-6">
          <Search
            className="absolute top-1/2 left-3 -translate-y-1/2 transform text-neutral-400"
            size={16}
          />
          <Input
            type="text"
            placeholder="Search integrations..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="pl-9 shadow-none"
          />
        </div>

        <div className="flex flex-col gap-4">
          {filteredIntegrations.length === 0 ? (
            <div className="py-12 text-center text-neutral-500">
              <Puzzle size={48} className="mx-auto mb-4 text-neutral-300" />
              <p className="text-sm">No integrations found</p>
              <p className="mt-1 text-xs text-neutral-400">
                Try a different search term
              </p>
            </div>
          ) : (
            filteredIntegrations.map((integration) => (
              <IntegrationCard
                key={integration.id}
                integration={integration}
                onConnect={handleConnect}
                onDisconnect={handleDisconnect}
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}

function IntegrationCard({
  integration,
  onConnect,
  onDisconnect,
}: {
  integration: Integration;
  onConnect: (integrationId: string) => void;
  onDisconnect: (integrationId: string) => void;
}) {
  const [isConnecting, setIsConnecting] = useState(false);

  const handleConnect = async () => {
    setIsConnecting(true);
    // Mock connect process - simulate API call
    await new Promise((resolve) => setTimeout(resolve, 1500));
    setIsConnecting(false);
    onConnect(integration.id);
  };

  const handleSync = async () => {
    // Mock sync process - simulate API call
    await new Promise((resolve) => setTimeout(resolve, 2000));
    console.log("Synced integration:", integration.id);
  };

  const handleReconnect = async () => {
    // Mock reconnect process - simulate API call
    await new Promise((resolve) => setTimeout(resolve, 1500));
    console.log("Reconnected integration:", integration.id);
  };

  const handleDisconnect = () => {
    onDisconnect(integration.id);
  };

  if (!integration.connected) {
    return (
      <div className="rounded-lg border border-neutral-200 p-4">
        <div className="flex items-start gap-4">
          <div className="mt-1">{getProviderIcon(integration.provider)}</div>
          <div className="flex-1">
            <h3 className="mb-1 text-sm font-medium">{integration.name}</h3>
            <p className="text-xs text-neutral-600">
              {integration.description}
            </p>
          </div>
          <Button
            size="sm"
            variant="outline"
            onClick={handleConnect}
            disabled={isConnecting}
            className="shrink-0"
          >
            {isConnecting ? (
              <>
                <Spinner size={14} className="mr-2" />
                Connecting...
              </>
            ) : (
              "Connect"
            )}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <ConnectedServiceCard
      icon={getProviderIcon(integration.provider)}
      title={integration.name}
      subtitle={integration.accountInfo}
      onSync={handleSync}
      onReconnect={handleReconnect}
      onDisconnect={handleDisconnect}
      connectedAt={integration.connectedAt}
      disconnectDialogTitle={`Disconnect ${integration.name}?`}
      disconnectDialogDescription={
        <>
          Are you sure you want to disconnect {integration.name}
          {integration.accountInfo && ` (${integration.accountInfo})`}? This
          integration will no longer sync with Char.
        </>
      }
    >
      <p className="text-xs text-neutral-600">{integration.description}</p>
    </ConnectedServiceCard>
  );
}
