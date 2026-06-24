import { Trans, useLingui } from "@lingui/react/macro";
import { useQuery } from "@tanstack/react-query";
import { fetch } from "@tauri-apps/plugin-http";
import { ExternalLinkIcon, PlusIcon, XIcon } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Input } from "@hypr/ui/components/ui/input";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@hypr/ui/components/ui/popover";
import { cn } from "@hypr/utils";

import type { TodoProvider } from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { useConnections } from "~/auth/useConnections";
import { openIntegrationUrl } from "~/shared/integration";
import * as settings from "~/store/tinybase/store/settings";

async function searchGitHubRepos(query: string): Promise<string[]> {
  const resp = await fetch(
    `https://api.github.com/search/repositories?q=${encodeURIComponent(query)}&per_page=6`,
  );
  if (!resp.ok) {
    return [];
  }
  const data = (await resp.json()) as { items: { full_name: string }[] };
  return (data.items ?? []).map((item) => item.full_name);
}

export function GitHubTodoProviderContent({
  config,
}: {
  config: TodoProvider;
}) {
  const { t } = useLingui();
  const auth = useAuth();
  const { isPaid, upgradeToPro } = useBillingAccess();
  const { data: connections } = useConnections(isPaid);
  const [showAddInput, setShowAddInput] = useState(false);
  const [inputValue, setInputValue] = useState("");
  const [debouncedInput, setDebouncedInput] = useState("");
  const [showSuggestions, setShowSuggestions] = useState(false);

  const providerConnections = useMemo(
    () =>
      connections?.filter(
        (c) => c.integration_id === config.nangoIntegrationId,
      ) ?? [],
    [connections, config.nangoIntegrationId],
  );

  const repository =
    settings.UI.useValue("todo_github_repository", settings.STORE_ID) ?? "";
  const normalizedRepository = repository.trim();
  const hasRepository = normalizedRepository.length > 0;

  const setRepository = settings.UI.useSetValueCallback(
    "todo_github_repository",
    (value: string) => value,
    [],
    settings.STORE_ID,
  );

  useEffect(() => {
    const id = setTimeout(() => setDebouncedInput(inputValue), 300);
    return () => clearTimeout(id);
  }, [inputValue]);

  const { data: suggestions = [] } = useQuery({
    queryKey: ["github-repo-search", debouncedInput],
    queryFn: () => searchGitHubRepos(debouncedInput),
    enabled: debouncedInput.trim().length >= 2,
    staleTime: 30_000,
  });

  function handleSelect(repo: string) {
    setRepository(repo);
    setShowAddInput(false);
    setInputValue("");
    setDebouncedInput("");
    setShowSuggestions(false);
  }

  function handleAdd() {
    const trimmed = inputValue.trim();
    if (isGitHubRepository(trimmed)) {
      handleSelect(trimmed);
    }
  }

  const isValidInput = isGitHubRepository(inputValue.trim());
  const hasSuggestions = showSuggestions && suggestions.length > 0;

  return (
    <div className="flex flex-col gap-4">
      <p className="text-muted-foreground text-xs">
        <Trans>Only public repositories are supported.</Trans>{" "}
        {!auth.session ? (
          <span>
            <Trans>Sign in for private repo access.</Trans>
          </span>
        ) : !isPaid ? (
          <button
            type="button"
            onClick={upgradeToPro}
            className="hover:text-muted-foreground underline transition-colors"
          >
            <Trans>Upgrade for private repos.</Trans>
          </button>
        ) : providerConnections.length === 0 ? (
          <button
            type="button"
            onClick={() =>
              openIntegrationUrl(
                config.nangoIntegrationId,
                undefined,
                "connect",
                "todo",
              )
            }
            className="hover:text-muted-foreground underline transition-colors"
          >
            <Trans>Connect GitHub for private repos.</Trans>
          </button>
        ) : (
          <button
            type="button"
            onClick={() =>
              openIntegrationUrl(
                config.nangoIntegrationId,
                providerConnections[0]?.connection_id,
                "disconnect",
                "todo",
              )
            }
            className="hover:text-muted-foreground underline transition-colors"
          >
            <Trans>Disconnect private repo access.</Trans>
          </button>
        )}
      </p>

      {hasRepository && !showAddInput ? (
        <div className="flex items-center gap-2">
          <span className="text-muted-foreground text-sm">
            {normalizedRepository}
          </span>
          <button
            type="button"
            onClick={() =>
              void openerCommands.openUrl(
                `https://github.com/${normalizedRepository}`,
                null,
              )
            }
            className="text-muted-foreground hover:text-muted-foreground transition-colors"
            aria-label={t`Open repository on GitHub`}
          >
            <ExternalLinkIcon className="size-3.5" />
          </button>
          <button
            type="button"
            onClick={() => setRepository("")}
            className="text-muted-foreground hover:text-muted-foreground transition-colors"
            aria-label={t`Remove repository`}
          >
            <XIcon className="size-3.5" />
          </button>
        </div>
      ) : null}

      {showAddInput ? (
        <Popover
          open={hasSuggestions}
          onOpenChange={(open) => !open && setShowSuggestions(false)}
        >
          <PopoverAnchor asChild>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                handleAdd();
              }}
              className="flex items-center gap-2"
            >
              <Input
                autoFocus
                className="flex-1"
                placeholder={t`Search or type owner/repo`}
                value={inputValue}
                onChange={(e) => {
                  setInputValue(e.target.value);
                  setShowSuggestions(true);
                }}
                onFocus={() => setShowSuggestions(true)}
              />
              <button
                type="submit"
                disabled={!isValidInput}
                className="text-muted-foreground hover:text-foreground text-xs underline transition-colors disabled:cursor-not-allowed disabled:opacity-40"
              >
                <Trans>Add</Trans>
              </button>
              <button
                type="button"
                onClick={() => {
                  setShowAddInput(false);
                  setInputValue("");
                  setDebouncedInput("");
                }}
                className="text-muted-foreground hover:text-muted-foreground text-xs underline transition-colors"
              >
                <Trans>Cancel</Trans>
              </button>
            </form>
          </PopoverAnchor>
          <PopoverContent
            className="p-1"
            align="start"
            onOpenAutoFocus={(e) => e.preventDefault()}
          >
            {suggestions.map((repo) => (
              <button
                key={repo}
                type="button"
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => handleSelect(repo)}
                className={cn([
                  "text-muted-foreground flex w-full items-center px-3 py-1.5 text-left text-sm",
                  "hover:bg-accent transition-colors",
                ])}
              >
                {repo}
              </button>
            ))}
          </PopoverContent>
        </Popover>
      ) : (
        <button
          type="button"
          onClick={() => setShowAddInput(true)}
          className="text-muted-foreground hover:text-foreground flex w-fit items-center gap-1 text-xs transition-colors"
        >
          <PlusIcon className="size-3" />
          {hasRepository ? (
            <Trans>Replace repository</Trans>
          ) : (
            <Trans>Add repository</Trans>
          )}
        </button>
      )}
    </div>
  );
}

function isGitHubRepository(value: string) {
  return /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(value);
}
