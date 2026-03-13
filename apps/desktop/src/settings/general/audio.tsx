import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { GripVertical } from "lucide-react";
import { Reorder } from "motion/react";
import { useEffect, useMemo, useState } from "react";

import {
  type AudioDevice,
  commands as audioPriorityCommands,
} from "@hypr/plugin-audio-priority";
import { cn } from "@hypr/utils";

export function Audio() {
  return (
    <div className="flex flex-col gap-6">
      <h2 className="font-serif text-lg font-semibold">Audio</h2>
      <div className="flex flex-col gap-6">
        <DeviceList direction="input" />
        <DeviceList direction="output" />
      </div>
    </div>
  );
}

function DeviceList({ direction }: { direction: "input" | "output" }) {
  const queryClient = useQueryClient();

  const { data: devices } = useQuery({
    queryKey: [`audio-${direction}-devices`],
    queryFn: async () => {
      const result =
        direction === "input"
          ? await audioPriorityCommands.listInputDevices()
          : await audioPriorityCommands.listOutputDevices();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
    refetchInterval: 3000,
  });

  const { data: priorities } = useQuery({
    queryKey: [`audio-${direction}-priorities`],
    queryFn: async () => {
      const result =
        direction === "input"
          ? await audioPriorityCommands.getInputPriorities()
          : await audioPriorityCommands.getOutputPriorities();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  const sortedDevices = useMemo(() => {
    if (!devices || !priorities) {
      return devices ?? [];
    }

    return [...devices].sort((a, b) => {
      const aIndex = priorities.indexOf(a.id);
      const bIndex = priorities.indexOf(b.id);
      const aPos = aIndex === -1 ? Infinity : aIndex;
      const bPos = bIndex === -1 ? Infinity : bIndex;
      return aPos - bPos;
    });
  }, [devices, priorities]);

  const [localDevices, setLocalDevices] = useState<AudioDevice[]>([]);

  useEffect(() => {
    setLocalDevices(sortedDevices);
  }, [sortedDevices]);

  const savePrioritiesMutation = useMutation({
    mutationFn: async (newPriorities: string[]) => {
      const saveResult =
        direction === "input"
          ? await audioPriorityCommands.saveInputPriorities(newPriorities)
          : await audioPriorityCommands.saveOutputPriorities(newPriorities);
      if (saveResult.status === "error") {
        throw new Error(saveResult.error);
      }

      if (newPriorities.length > 0) {
        const setResult =
          direction === "input"
            ? await audioPriorityCommands.setDefaultInputDevice(
                newPriorities[0],
              )
            : await audioPriorityCommands.setDefaultOutputDevice(
                newPriorities[0],
              );
        if (setResult.status === "error") {
          throw new Error(setResult.error);
        }
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: [`audio-${direction}-priorities`],
      });
      queryClient.invalidateQueries({
        queryKey: [`audio-${direction}-devices`],
      });
    },
  });

  const handleReorder = (reordered: AudioDevice[]) => {
    setLocalDevices(reordered);
    const newPriorities = reordered.map((d) => d.id);
    savePrioritiesMutation.mutate(newPriorities);
  };

  if (!localDevices.length) {
    return null;
  }

  return (
    <div>
      <h3 className="mb-2 text-sm font-medium">
        {direction === "input" ? "Input devices" : "Output devices"}
      </h3>
      <p className="mb-3 text-xs text-neutral-500">
        {direction === "input"
          ? "Drag to set microphone priority. Top device will be auto-selected."
          : "Drag to set speaker priority. Top device will be auto-selected."}
      </p>
      <Reorder.Group
        axis="y"
        values={localDevices}
        onReorder={handleReorder}
        className="flex flex-col gap-1"
      >
        {localDevices.map((device, index) => (
          <DeviceItem
            key={device.id}
            device={device}
            rank={index + 1}
            isTop={index === 0}
          />
        ))}
      </Reorder.Group>
    </div>
  );
}

function DeviceItem({
  device,
  rank,
  isTop,
}: {
  device: AudioDevice;
  rank: number;
  isTop: boolean;
}) {
  return (
    <Reorder.Item
      value={device}
      className={cn([
        "flex cursor-grab items-center gap-2 rounded-lg px-3 py-2 active:cursor-grabbing",
        "border transition-colors",
        isTop
          ? "border-neutral-300 bg-neutral-100"
          : "border-neutral-200 bg-neutral-50 hover:bg-neutral-100",
      ])}
    >
      <GripVertical className="h-4 w-4 shrink-0 text-neutral-400" />
      <span className="w-4 text-xs text-neutral-400">{rank}</span>
      <span className={cn(["flex-1 truncate text-sm", isTop && "font-medium"])}>
        {device.name}
      </span>
      {isTop && <span className="text-xs text-neutral-500">Active</span>}
    </Reorder.Item>
  );
}
