import { createContext, useContext } from "react";

export type AttachmentResolution = {
  path: string;
  src: string;
};

export type AttachmentResolver = (
  attachmentId: string,
) => AttachmentResolution | null;

export const AttachmentResolverContext =
  createContext<AttachmentResolver | null>(null);
export const AttachmentEditingContext = createContext(true);

export function useAttachmentResolver() {
  return useContext(AttachmentResolverContext);
}

export function useAttachmentEditingEnabled() {
  return useContext(AttachmentEditingContext);
}
