import { Trans } from "@lingui/react/macro";
import { Lock, LockOpen } from "@phosphor-icons/react";
import { useCallback } from "react";

import { DropdownMenuItem } from "@anlg/ui/components/ui/dropdown-menu";

import { isLockedFlag } from "~/lock/flag";
import { setSessionLocked } from "~/lock/notes";
import { useAppLock } from "~/lock/store";
import { useSession } from "~/session/queries";

export function LockNote({ sessionId }: { sessionId: string }) {
  const session = useSession(sessionId);
  const locked = isLockedFlag(session?.locked);
  const available = useAppLock((state) => state.available) === true;
  const authenticating = useAppLock((state) => state.authenticating);

  const handleToggle = useCallback(() => {
    void setSessionLocked(sessionId, !locked);
  }, [locked, sessionId]);

  if (!available) return null;

  return (
    <DropdownMenuItem
      onClick={(e) => {
        e.preventDefault();
        handleToggle();
      }}
      disabled={authenticating || session == null}
      className="cursor-pointer"
    >
      {locked ? <LockOpen /> : <Lock />}
      <span>
        {locked ? <Trans>Unlock Note</Trans> : <Trans>Lock Note</Trans>}
      </span>
    </DropdownMenuItem>
  );
}
