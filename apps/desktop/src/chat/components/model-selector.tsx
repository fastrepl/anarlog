import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation } from "@tanstack/react-query";
import { useMemo, useRef, useState } from "react";

import { CaretDown, Check, MagnifyingGlass } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuPortal,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { toast } from "@anlg/ui/components/ui/toast";

import {
  useLLMConnection,
  useLLMSelection,
  useModelMetadata,
} from "~/ai/hooks";
import { useConfiguredLlmProviders } from "~/settings/ai/llm/configured-providers";
import { type Provider, PROVIDERS } from "~/settings/ai/llm/shared";
import { ProviderIconSlot } from "~/settings/ai/shared";
import type { ListModelsResult } from "~/settings/ai/shared/list-common";
import { displayLlmModelId } from "~/settings/ai/shared/model-display";
import { getConfiguredProviders } from "~/settings/ai/shared/selection";
import { setSettingValues } from "~/settings/queries";

export function ChatModelSelector({
  disabled = false,
}: {
  disabled?: boolean;
}) {
  const { t } = useLingui();
  const selection = useLLMSelection("chat");
  const { status } = useLLMConnection("chat");
  const [open, setOpen] = useState(false);
  const saveSelection = useMutation({
    mutationFn: ({ provider, model }: { provider: string; model: string }) =>
      setSettingValues({ chat_llm_provider: provider, chat_llm_model: model }),
    onSuccess: () => setOpen(false),
    onError: () => toast.error(t`Could not save the chat model. Try again.`),
  });
  const provider = PROVIDERS.find(({ id }) => id === selection.providerId);
  const modelName = selection.modelId
    ? displayLlmModelId(selection.providerId ?? "", selection.modelId)
    : t`Select a model`;
  const label = provider ? `${provider.displayName}, ${modelName}` : modelName;
  const isDisabled =
    disabled ||
    saveSelection.isPending ||
    (status.status === "pending" &&
      (status.reason === "managed_disabled" ||
        status.reason === "settings_loading"));

  return (
    <DropdownMenu open={open && !isDisabled} onOpenChange={setOpen}>
      <DropdownMenuTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          aria-label={t`Chat model: ${label}`}
          disabled={isDisabled}
          title={label}
          className="text-muted-foreground h-7 max-w-40 gap-1 rounded-md px-2 text-xs font-normal"
        >
          {provider && <ProviderIconSlot>{provider.icon}</ProviderIconSlot>}
          <span className="truncate">{modelName}</span>
          {status.status === "error" && (
            <span className="shrink-0">
              <Trans>Unavailable</Trans>
            </span>
          )}
          <CaretDown className="size-3 shrink-0" />
        </Button>
      </DropdownMenuTrigger>
      {open && !isDisabled && (
        <ModelMenu
          selection={selection}
          onSelect={(provider, model) =>
            saveSelection.mutate({ provider, model })
          }
        />
      )}
    </DropdownMenu>
  );
}

function ModelMenu({
  selection,
  onSelect,
}: {
  selection: ReturnType<typeof useLLMSelection>;
  onSelect: (provider: string, model: string) => void;
}) {
  const { t } = useLingui();
  const { providers, isReady } = useConfiguredLlmProviders(
    selection.providerId,
  );
  const [query, setQuery] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const options = getConfiguredProviders(PROVIDERS, providers);
  const emptyMessage = isReady
    ? t`Connect a provider in Intelligence settings to choose a model.`
    : t`Loading providers...`;

  return (
    <DropdownMenuContent
      variant="app"
      align="end"
      side="top"
      className="w-68 max-w-[calc(100vw-2rem)]"
      onFocus={(event) => {
        if (event.target === event.currentTarget) {
          event.preventDefault();
          searchRef.current?.focus();
        }
      }}
    >
      <AppFloatingPanel className="overflow-hidden">
        <div className="flex items-center gap-2 px-3 py-2">
          <MagnifyingGlass
            className="text-muted-foreground size-4 shrink-0"
            aria-hidden="true"
          />
          <input
            ref={searchRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            aria-label={t`Search models`}
            placeholder={t`Search models`}
            className="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-sm outline-none"
            onKeyDown={(event) => {
              if (event.key === "ArrowDown") {
                event.preventDefault();
                event.currentTarget
                  .closest('[role="menu"]')
                  ?.querySelector<HTMLElement>(
                    '[role="menuitem"]:not([aria-disabled="true"])',
                  )
                  ?.focus();
              }
              if (event.key !== "Escape") event.stopPropagation();
            }}
          />
        </div>
        <div className="max-h-80 overflow-y-auto p-1">
          {options.map((provider) => (
            <ProviderModels
              key={provider.id}
              provider={provider}
              listModels={providers[provider.id].listModels}
              modelScope={providers[provider.id].modelScope}
              query={query.trim()}
              selectedModel={
                selection.providerId === provider.id
                  ? selection.modelId
                  : undefined
              }
              onSelect={(model) => onSelect(provider.id, model)}
            />
          ))}
          <div className="text-muted-foreground hidden px-2 py-3 text-center text-sm only:block">
            {options.length > 0 ? t`No models found.` : emptyMessage}
          </div>
        </div>
      </AppFloatingPanel>
    </DropdownMenuContent>
  );
}

function ProviderModels({
  provider,
  listModels,
  modelScope,
  selectedModel,
  query,
  onSelect,
}: {
  provider: Provider;
  listModels?: () => Promise<ListModelsResult>;
  modelScope?: string;
  selectedModel?: string;
  query: string;
  onSelect: (model: string) => void;
}) {
  const normalizedQuery = query.toLocaleLowerCase();
  const [open, setOpen] = useState(false);
  const { data, isLoading, isError, refetch } = useModelMetadata(
    provider.id,
    listModels,
    {
      enabled: open || normalizedQuery.length > 0,
      scope: modelScope,
    },
  );
  const providerMatches = `${provider.id} ${provider.displayName}`
    .toLocaleLowerCase()
    .includes(normalizedQuery);
  const models = useMemo(() => {
    const ids = data?.models ?? [];
    const available =
      selectedModel && !ids.includes(selectedModel)
        ? [selectedModel, ...ids]
        : ids;
    return available
      .filter((id) => provider.id !== "anarlog" || id !== "Auto")
      .map((id) => {
        const label = displayLlmModelId(provider.id, id);
        return { id, label, searchText: `${id} ${label}`.toLocaleLowerCase() };
      });
  }, [data?.models, provider.id, selectedModel]);
  let options =
    normalizedQuery && !providerMatches
      ? models.filter(({ searchText }) => searchText.includes(normalizedQuery))
      : models;
  if (
    provider.id === "custom" &&
    normalizedQuery &&
    !models.some(({ id }) => id.toLocaleLowerCase() === normalizedQuery)
  ) {
    options = [
      { id: query, label: query, searchText: normalizedQuery },
      ...options,
    ];
  }

  if (
    normalizedQuery &&
    !providerMatches &&
    !isLoading &&
    !isError &&
    options.length === 0
  )
    return null;

  const modelItems =
    normalizedQuery || open ? (
      <ModelItems
        key={query}
        models={options}
        selectedModel={selectedModel}
        isLoading={isLoading}
        isError={isError}
        onRetry={() => {
          void refetch();
        }}
        onSelect={onSelect}
      />
    ) : null;

  if (normalizedQuery) {
    return (
      <DropdownMenuGroup
        aria-label={provider.displayName}
        className="border-app-floating-border not-first:mt-1 not-first:border-t not-first:pt-1"
      >
        <DropdownMenuLabel className="text-muted-foreground flex items-center gap-2">
          <ProviderIconSlot>{provider.icon}</ProviderIconSlot>
          {provider.displayName}
        </DropdownMenuLabel>
        {modelItems}
      </DropdownMenuGroup>
    );
  }

  return (
    <DropdownMenuSub open={open} onOpenChange={setOpen}>
      <DropdownMenuSubTrigger>
        <ProviderIconSlot>{provider.icon}</ProviderIconSlot>
        {provider.displayName}
      </DropdownMenuSubTrigger>
      <DropdownMenuPortal>
        <DropdownMenuSubContent
          variant="app"
          className="w-72 max-w-[calc(100vw-2rem)]"
        >
          <AppFloatingPanel className="max-h-[min(20rem,var(--radix-dropdown-menu-content-available-height))] overflow-y-auto p-1">
            {modelItems}
          </AppFloatingPanel>
        </DropdownMenuSubContent>
      </DropdownMenuPortal>
    </DropdownMenuSub>
  );
}

function ModelItems({
  models,
  selectedModel,
  isLoading,
  isError,
  onRetry,
  onSelect,
}: {
  models: { id: string; label: string }[];
  selectedModel?: string;
  isLoading: boolean;
  isError: boolean;
  onRetry: () => void;
  onSelect: (model: string) => void;
}) {
  // ponytail: 50 rows per batch; virtualize if users routinely browse whole catalogs.
  const batchSize = 50;
  const [visibleCount, setVisibleCount] = useState(batchSize);
  return (
    <>
      {isLoading && (
        <DropdownMenuItem disabled>
          <Trans>Loading models...</Trans>
        </DropdownMenuItem>
      )}
      {isError && (
        <DropdownMenuItem
          onSelect={(event) => {
            event.preventDefault();
            onRetry();
          }}
        >
          <Trans>Could not load models. Retry</Trans>
        </DropdownMenuItem>
      )}
      {!isLoading && !isError && models.length === 0 && (
        <DropdownMenuItem disabled>
          <Trans>No models available.</Trans>
        </DropdownMenuItem>
      )}
      {models.slice(0, visibleCount).map(({ id, label }) => (
        <DropdownMenuItem
          key={id}
          onSelect={(event) => {
            event.preventDefault();
            onSelect(id);
          }}
          aria-current={selectedModel === id ? "true" : undefined}
        >
          <span className="min-w-0 flex-1 truncate" title={id}>
            {label}
          </span>
          {selectedModel === id && (
            <Check className="size-4 shrink-0" aria-hidden="true" />
          )}
        </DropdownMenuItem>
      ))}
      {models.length > visibleCount && (
        <DropdownMenuItem
          onSelect={(event) => {
            event.preventDefault();
            setVisibleCount((count) => count + batchSize);
          }}
        >
          <Trans>Show more models</Trans>
        </DropdownMenuItem>
      )}
    </>
  );
}
