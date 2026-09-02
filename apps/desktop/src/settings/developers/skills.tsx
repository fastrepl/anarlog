import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import { Claude, Cursor, OpenAI } from "@lobehub/icons";
import { CaretDown, Check, CircleNotch } from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  appFloatingMenuPanelClassName,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { commands, type SkillAgent } from "~/types/tauri.gen";

const SKILL_AGENTS_QUERY_KEY = ["skill-agents"] as const;

function OpenCodeIcon() {
  return (
    <svg viewBox="0 0 300 300" fill="none">
      <g transform="translate(30 0)">
        <path
          className="fill-[#CFCECD] dark:fill-[#4B4646]"
          d="M180 240H60V120h120z"
        />
        <path
          className="fill-[#211E1E] dark:fill-[#F1ECEC]"
          fillRule="evenodd"
          d="M180 60H60v180h120V60ZM240 300H0V0h240v300Z"
        />
      </g>
    </svg>
  );
}

function SkillAgentIcon({ agent }: { agent: SkillAgent }) {
  let icon;
  switch (agent) {
    case "claude_code":
      icon = <Claude.Color size={16} />;
      break;
    case "codex":
      icon = <OpenAI size={16} />;
      break;
    case "cursor":
      icon = <Cursor size={16} />;
      break;
    case "opencode":
      icon = <OpenCodeIcon />;
      break;
  }

  return (
    <span
      aria-hidden
      className="flex size-4 shrink-0 items-center justify-center [&>svg]:size-4"
    >
      {icon}
    </span>
  );
}

async function loadSkillAgents() {
  const result = await commands.listSkillAgents();
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

async function installSkill(agent: SkillAgent) {
  const result = await commands.installAgentSkill(agent);
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

export function SkillsRow() {
  const queryClient = useQueryClient();
  const agentsQuery = useQuery({
    queryKey: SKILL_AGENTS_QUERY_KEY,
    queryFn: loadSkillAgents,
  });
  const installMutation = useMutation({
    mutationFn: async (agents: SkillAgent[]) => {
      const statuses = [];
      for (const agent of agents) {
        statuses.push(await installSkill(agent));
      }
      return statuses;
    },
    onSuccess: (statuses) => {
      void queryClient.invalidateQueries({ queryKey: SKILL_AGENTS_QUERY_KEY });
      sonnerToast.success(
        statuses.length === 1
          ? t`Anarlog skill added to ${statuses[0].displayName}`
          : t`Anarlog skill added to ${statuses.length} agents`,
      );
    },
    onError: (error) => {
      void queryClient.invalidateQueries({ queryKey: SKILL_AGENTS_QUERY_KEY });
      sonnerToast.error(error.message);
    },
  });

  const agents = agentsQuery.data ?? [];
  const detected = agents.filter((agent) => agent.detected);

  return (
    <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
      <div className="min-w-0">
        <h3 className="text-sm font-medium">
          <Trans>Agent skills</Trans>
        </h3>
        <p className="text-muted-foreground mt-1 text-xs">
          <Trans>
            Teach coding agents when and how to use the Anarlog CLI and MCP
          </Trans>
        </p>
      </div>
      <div className="flex shrink-0 gap-2">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={agents.length === 0 || installMutation.isPending}
            >
              {installMutation.isPending ? (
                <CircleNotch className="size-3.5 animate-spin" />
              ) : (
                <>
                  <Trans>Add skill to…</Trans>
                  <CaretDown className="size-3.5" />
                </>
              )}
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent variant="app" align="end" className="w-52">
            <AppFloatingPanel className={appFloatingMenuPanelClassName}>
              <DropdownMenuItem
                disabled={detected.length === 0}
                onClick={() =>
                  installMutation.mutate(detected.map((agent) => agent.agent))
                }
              >
                <Trans>Install to all agents</Trans>
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              {agents.map((agent) => (
                <DropdownMenuItem
                  key={agent.agent}
                  disabled={!agent.detected}
                  onClick={() => installMutation.mutate([agent.agent])}
                >
                  <SkillAgentIcon agent={agent.agent} />
                  {agent.displayName}
                  {agent.installed && (
                    <Check
                      aria-label={t`Skill installed`}
                      className="ml-auto size-3.5"
                    />
                  )}
                </DropdownMenuItem>
              ))}
            </AppFloatingPanel>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  );
}
