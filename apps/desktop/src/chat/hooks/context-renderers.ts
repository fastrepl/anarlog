import { useDisplayEntityRenderer as useDisplayEntityRendererBase } from "~/session/hooks/storage";
import { useResolveContextRef as useResolveContextRefBase } from "~/session/hooks/storage";

export function useDisplayEntityRenderer() {
  return useDisplayEntityRendererBase();
}

export function useResolveContextRef() {
  return useResolveContextRefBase();
}
