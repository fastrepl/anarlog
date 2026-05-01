import path from "path";

import { createAgentNode, loadPrompt } from "@hypr/agent-core";

import { tools } from "../tools";

const prompt = loadPrompt(path.join(import.meta.dirname, ".."));

export const agentNode = createAgentNode(prompt, tools);
