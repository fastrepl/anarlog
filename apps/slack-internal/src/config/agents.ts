import { agent as designerAgent } from "@hypr/agent-designer";
import type { CompiledAgentGraph } from "@hypr/agent-support";
import { agent as internalAgent } from "@hypr/agent-support";

import { env } from "../env";

export interface AgentConfig {
  agent: CompiledAgentGraph;
  name: string;
  description: string;
}

export const agents = {
  internal: {
    agent: internalAgent,
    name: "Internal Ops Agent",
    description:
      "Handles internal operations, Stripe, Loops, Supabase, PostHog",
  },
  designer: {
    agent: designerAgent,
    name: "Designer Agent",
    description: "Handles UI/UX design tasks using Magic Patterns",
  },
} as const satisfies Record<string, AgentConfig>;

export type AgentType = keyof typeof agents;

function parseChannelAgentMap(): Record<string, AgentType> {
  try {
    const parsed = JSON.parse(env.AGENT_CHANNEL_MAP) as Record<string, string>;
    const validMap: Record<string, AgentType> = {};

    for (const [channelId, agentType] of Object.entries(parsed)) {
      if (agentType in agents) {
        validMap[channelId] = agentType as AgentType;
      } else {
        console.warn(
          `Invalid agent type "${agentType}" for channel "${channelId}". Valid types: ${Object.keys(agents).join(", ")}`,
        );
      }
    }

    if (Object.keys(validMap).length > 0) {
      console.log(`Loaded channel-agent mappings: ${JSON.stringify(validMap)}`);
    }

    return validMap;
  } catch (error) {
    console.error("Failed to parse AGENT_CHANNEL_MAP:", error);
    return {};
  }
}

const channelAgentMap: Record<string, AgentType> = parseChannelAgentMap();

const defaultAgent: AgentType = "internal";

export function setChannelAgent(channelId: string, agentType: AgentType): void {
  channelAgentMap[channelId] = agentType;
}

export function getAgentForChannel(channelId: string): AgentConfig {
  const agentType = channelAgentMap[channelId] ?? defaultAgent;
  return agents[agentType];
}

export function getAgentTypeForChannel(channelId: string): AgentType {
  return channelAgentMap[channelId] ?? defaultAgent;
}
