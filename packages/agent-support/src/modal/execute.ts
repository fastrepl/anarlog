import { sandboxManager } from "./sandbox";

const EXECUTION_TIMEOUT_MS = 100_000;

export interface ExecutionResult {
  success: boolean;
  stdout: string;
  stderr: string;
  exitCode: number;
  executionTimeMs: number;
}

export function formatExecutionResult(result: ExecutionResult): string {
  const lines = [
    `success: ${result.success}`,
    `exitCode: ${result.exitCode}`,
    `executionTimeMs: ${result.executionTimeMs}`,
  ];
  if (result.stdout) lines.push(`stdout:\n${result.stdout}`);
  if (result.stderr) lines.push(`stderr:\n${result.stderr}`);
  return lines.join("\n");
}

function wrapCodeWithExit(code: string): string {
  return `
(async () => {
  try {
${code}
  } finally {
    process.exit(0);
  }
})();
`.trim();
}

export async function executeCode(code: string): Promise<ExecutionResult> {
  const startTime = Date.now();
  const sandbox = await sandboxManager.create();
  const wrappedCode = wrapCodeWithExit(code);

  try {
    const process = await sandbox.exec(["bun", "-e", wrappedCode], {
      stdout: "pipe",
      stderr: "pipe",
      timeoutMs: EXECUTION_TIMEOUT_MS,
    });

    const [stdout, stderr] = await Promise.all([
      process.stdout.readText(),
      process.stderr.readText(),
    ]);

    const exitCode = await process.wait();

    return {
      success: exitCode === 0,
      stdout: stdout.trim(),
      stderr: stderr.trim(),
      exitCode,
      executionTimeMs: Date.now() - startTime,
    };
  } catch (error) {
    return {
      success: false,
      stdout: "",
      stderr: error instanceof Error ? error.message : String(error),
      exitCode: 1,
      executionTimeMs: Date.now() - startTime,
    };
  } finally {
    await sandboxManager.release(sandbox);
  }
}
