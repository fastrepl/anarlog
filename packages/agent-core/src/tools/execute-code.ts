import { tool } from "@langchain/core/tools";
import { z } from "zod";

export type ExecuteCodeArgs = z.infer<typeof executeCodeArgsSchema>;

export const executeCodeArgsSchema = z.object({
  code: z.string().describe("The code to execute"),
  isMutating: z
    .boolean()
    .optional()
    .describe(
      "True if this operation creates, updates, or deletes data. False for read-only operations.",
    ),
});

export interface ExecuteCodeFunction {
  (code: string): Promise<ExecuteCodeResult>;
}

export interface ExecuteCodeResult {
  success: boolean;
  stdout: string;
  stderr: string;
  exitCode: number;
  executionTimeMs: number;
}

export function formatExecutionResult(result: ExecuteCodeResult): string {
  const lines = [
    `success: ${result.success}`,
    `exitCode: ${result.exitCode}`,
    `executionTimeMs: ${result.executionTimeMs}`,
  ];
  if (result.stdout) lines.push(`stdout:\n${result.stdout}`);
  if (result.stderr) lines.push(`stderr:\n${result.stderr}`);
  return lines.join("\n");
}

let executeCodeFn: ExecuteCodeFunction | null = null;

export function setExecuteCodeFunction(fn: ExecuteCodeFunction): void {
  executeCodeFn = fn;
}

export const executeCodeTool = tool(
  async ({ code }: ExecuteCodeArgs) => {
    if (!executeCodeFn) {
      return "executeCode function not configured. Call setExecuteCodeFunction first.";
    }
    const result = await executeCodeFn(code);
    return formatExecutionResult(result);
  },
  {
    name: "executeCode",
    description:
      "Execute TypeScript/JavaScript code in a sandboxed environment",
    schema: executeCodeArgsSchema,
  },
);
