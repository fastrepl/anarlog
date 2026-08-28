import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import { CaretDown, Check, CircleNotch } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { colors, media } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { commands, type SkillAgent } from "~/types/tauri.gen";

const SKILL_AGENTS_QUERY_KEY = ["skill-agents"] as const;

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
    <div {...stylex.props(styles.row)}>
      <div {...stylex.props(styles.copy)}>
        <h3 {...stylex.props(styles.title)}>
          <Trans>Agent skills</Trans>
        </h3>
        <p {...stylex.props(styles.description)}>
          <Trans>
            Teach coding agents when and how to use the Anarlog CLI and MCP
          </Trans>
        </p>
      </div>
      <div {...stylex.props(styles.actions)}>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={agents.length === 0 || installMutation.isPending}
            >
              {installMutation.isPending ? (
                <CircleNotch {...stylex.props(styles.spinner)} />
              ) : (
                <>
                  <Trans>Add skill to…</Trans>
                  <CaretDown {...stylex.props(styles.icon)} />
                </>
              )}
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent variant="app" align="end" sx={styles.menu}>
            <AppFloatingPanel sx={styles.menuPanel}>
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
                  {agent.displayName}
                  {agent.installed && (
                    <Check
                      aria-label={t`Skill installed`}
                      {...stylex.props(styles.check)}
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

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  actions: {
    display: "flex",
    flexShrink: 0,
    gap: "0.5rem",
  },
  check: {
    height: "0.875rem",
    marginLeft: "auto",
    width: "0.875rem",
  },
  copy: {
    minWidth: 0,
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
    marginTop: "0.25rem",
  },
  icon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  menu: {
    width: "13rem",
  },
  menuPanel: {
    padding: "0.25rem",
  },
  row: {
    alignItems: {
      default: "stretch",
      [media.sm]: "center",
    },
    display: "flex",
    flexDirection: {
      default: "column",
      [media.sm]: "row",
    },
    gap: "1rem",
    justifyContent: {
      default: "flex-start",
      [media.sm]: "space-between",
    },
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "0.875rem",
    width: "0.875rem",
  },
  title: {
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
  },
});
