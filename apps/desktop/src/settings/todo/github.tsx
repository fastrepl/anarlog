import { Trans, useLingui } from "@lingui/react/macro";
import { ArrowSquareOut, CircleNotch, Plus, X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useQuery } from "@tanstack/react-query";
import { fetch } from "@tauri-apps/plugin-http";
import { useEffect, useMemo, useState } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Input } from "@anlg/ui/components/ui/input";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@anlg/ui/components/ui/popover";

import type { TodoProvider } from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { useConnections } from "~/auth/useConnections";
import { useSetSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";
import { useOpenIntegrationUrl } from "~/shared/integration";

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
  const { isPaid, upgradeToPro, isUpgradingToPro } = useBillingAccess();
  const { data: connections } = useConnections(isPaid);
  const { openIntegration, openingAction } = useOpenIntegrationUrl();
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

  const repository = useConfigValue("todo_github_repository") ?? "";
  const normalizedRepository = repository.trim();
  const hasRepository = normalizedRepository.length > 0;

  const setRepository = useSetSettingValue("todo_github_repository");

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
    <div {...stylex.props(styles.container)}>
      <p {...stylex.props(styles.hint)}>
        <Trans>Only public repositories are supported.</Trans>{" "}
        {!auth.session ? (
          <span>
            <Trans>Sign in for private repo access.</Trans>
          </span>
        ) : !isPaid ? (
          <button
            type="button"
            onClick={upgradeToPro}
            disabled={isUpgradingToPro}
            {...stylex.props(styles.inlineLink)}
          >
            {isUpgradingToPro && (
              <CircleNotch
                {...stylex.props(styles.spinner)}
                aria-hidden="true"
              />
            )}
            <Trans>Upgrade for private repos.</Trans>
          </button>
        ) : providerConnections.length === 0 ? (
          <button
            type="button"
            onClick={() =>
              openIntegration({
                nangoIntegrationId: config.nangoIntegrationId,
                action: "connect",
                returnTo: "todo",
              })
            }
            disabled={openingAction !== null}
            {...stylex.props(styles.inlineLink)}
          >
            {openingAction === "connect" && (
              <CircleNotch
                {...stylex.props(styles.spinner)}
                aria-hidden="true"
              />
            )}
            <Trans>Connect GitHub for private repos.</Trans>
          </button>
        ) : (
          <button
            type="button"
            onClick={() =>
              openIntegration({
                nangoIntegrationId: config.nangoIntegrationId,
                connectionId: providerConnections[0]?.connection_id,
                action: "disconnect",
                returnTo: "todo",
              })
            }
            disabled={openingAction !== null}
            {...stylex.props(styles.inlineLink)}
          >
            {openingAction === "disconnect" && (
              <CircleNotch
                {...stylex.props(styles.spinner)}
                aria-hidden="true"
              />
            )}
            <Trans>Disconnect private repo access.</Trans>
          </button>
        )}
      </p>

      {hasRepository && !showAddInput ? (
        <div {...stylex.props(styles.repository)}>
          <span {...stylex.props(styles.repositoryName)}>
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
            {...stylex.props(styles.iconButton)}
            aria-label={t`Open repository on GitHub`}
          >
            <ArrowSquareOut {...stylex.props(styles.icon)} />
          </button>
          <button
            type="button"
            onClick={() => setRepository("")}
            {...stylex.props(styles.iconButton)}
            aria-label={t`Remove repository`}
          >
            <X {...stylex.props(styles.icon)} />
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
              {...stylex.props(styles.form)}
            >
              <Input
                autoFocus
                sx={styles.input}
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
                {...stylex.props(styles.formAction)}
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
                {...stylex.props(styles.formAction, styles.cancelAction)}
              >
                <Trans>Cancel</Trans>
              </button>
            </form>
          </PopoverAnchor>
          <PopoverContent
            sx={styles.popover}
            align="start"
            onOpenAutoFocus={(e) => e.preventDefault()}
          >
            {suggestions.map((repo) => (
              <button
                key={repo}
                type="button"
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => handleSelect(repo)}
                {...stylex.props(styles.suggestion)}
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
          {...stylex.props(styles.addButton)}
        >
          <Plus {...stylex.props(styles.plus)} />
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

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  addButton: {
    alignItems: "center",
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.25rem",
    lineHeight: "1rem",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "fit-content",
  },
  cancelAction: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.mutedForeground,
    },
  },
  container: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  form: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  formAction: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    cursor: {
      default: "pointer",
      ":disabled": "not-allowed",
    },
    fontSize: "0.75rem",
    lineHeight: "1rem",
    opacity: {
      default: 1,
      ":disabled": 0.4,
    },
    textDecorationLine: "underline",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  hint: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  icon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  iconButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.mutedForeground,
    },
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  inlineLink: {
    alignItems: "center",
    color: {
      default: colors.mutedForeground,
      ":hover": colors.mutedForeground,
    },
    display: "inline-flex",
    gap: "0.25rem",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    textDecorationLine: "underline",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  input: {
    flex: "1",
  },
  plus: {
    height: "0.75rem",
    width: "0.75rem",
  },
  popover: {
    padding: "0.25rem",
  },
  repository: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  repositoryName: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "0.75rem",
    width: "0.75rem",
  },
  suggestion: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    paddingBlock: "0.375rem",
    paddingInline: "0.75rem",
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
});
