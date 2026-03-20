import { createContext, useContext } from "react";

interface BlogTocContextType {
  toc: Array<{ id: string; text: string; level: number }>;
  activeId: string | null;
  setToc: (toc: Array<{ id: string; text: string; level: number }>) => void;
  setActiveId: (id: string | null) => void;
  scrollToHeading: (id: string) => void;
}

export const BlogTocContext = createContext<BlogTocContextType | null>(null);

export function useBlogToc() {
  return useContext(BlogTocContext);
}
