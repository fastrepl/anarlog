// iPhone 15 Pro (iPhone16,1/16,2) introduced the Action Button; every later iPhone has one.
export function modelHasActionButton(modelId: string | null): boolean {
  const match = /^iPhone(\d+),(\d+)$/.exec(modelId ?? "");
  if (!match) return false;
  const major = Number(match[1]);
  const minor = Number(match[2]);
  return major >= 17 || (major === 16 && (minor === 1 || minor === 2));
}
