import { t } from "@lingui/core/macro";

import { toast } from "@anlg/ui/components/ui/toast";

export async function copyText(value: string, message: string) {
  try {
    await navigator.clipboard.writeText(value);
    toast.success(message);
    return true;
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : t`Could not copy to clipboard`,
    );
    return false;
  }
}
