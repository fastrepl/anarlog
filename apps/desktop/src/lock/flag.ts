export function isLockedFlag(value: unknown): boolean {
  return value === true || Number(value) === 1;
}
