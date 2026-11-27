// AuthProvider owns this identity, including its offline session restoration.
// It selects local settings and cannot authorize cloud access.
let activeAccountId: string | null = null;

export function setActiveAccountId(accountId: string | null): void {
  activeAccountId = accountId;
}

export function getActiveAccountId(): string | null {
  return activeAccountId;
}
