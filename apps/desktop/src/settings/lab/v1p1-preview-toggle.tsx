import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { relaunch } from "@tauri-apps/plugin-process";

import { Switch } from "@hypr/ui/components/ui/switch";

import { commands } from "~/types/tauri.gen";

export function V1p1PreviewToggle() {
  const queryClient = useQueryClient();

  const { data: enabled = false } = useQuery({
    queryKey: ["char_v1p1_preview"],
    queryFn: async () => {
      const result = await commands.getCharV1p1Preview();
      return result.status === "ok" ? result.data : false;
    },
  });

  const mutation = useMutation({
    mutationFn: async (v: boolean) => {
      await commands.setCharV1p1Preview(v);
    },
    onSuccess: async (_data, v) => {
      queryClient.setQueryData(["char_v1p1_preview"], v);
      await relaunch();
    },
  });

  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex-1">
        <h3 className="mb-1 text-sm font-medium">New Layout</h3>
        <p className="text-xs text-neutral-600">
          Try the new layout experience. The app will restart to apply.
        </p>
      </div>
      <Switch
        checked={enabled}
        onCheckedChange={(v) => mutation.mutate(v)}
        disabled={mutation.isPending}
      />
    </div>
  );
}
