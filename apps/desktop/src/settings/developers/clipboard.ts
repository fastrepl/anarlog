import { sonnerToast } from "@anlg/ui/components/ui/toast";

export async function copyText(value: string, message: string) {
  try {
    await navigator.clipboard.writeText(value);
    sonnerToast.success(message);
    return true;
  } catch (error) {
    sonnerToast.error(
      error instanceof Error ? error.message : "Could not copy to clipboard",
    );
    return false;
  }
}
