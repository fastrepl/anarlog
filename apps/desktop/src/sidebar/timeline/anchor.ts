import {
  type DependencyList,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";

export function useAnchor() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [isAnchorVisible, setIsAnchorVisible] = useState(true);
  const [isScrolledPastAnchor, setIsScrolledPastAnchor] = useState(false);
  const [anchorNode, setAnchorNode] = useState<HTMLDivElement | null>(null);

  const registerAnchor = useCallback((node: HTMLDivElement | null) => {
    setAnchorNode((previousNode) =>
      previousNode === node ? previousNode : node,
    );
  }, []);

  const scrollToAnchor = useCallback(() => {
    const container = containerRef.current;
    if (!container || !anchorNode) {
      return;
    }

    const containerRect = container.getBoundingClientRect();
    const anchorRect = anchorNode.getBoundingClientRect();
    const anchorCenter =
      anchorRect.top -
      containerRect.top +
      container.scrollTop +
      anchorRect.height / 2;
    const targetScrollTop = Math.max(
      anchorCenter - container.clientHeight / 2,
      0,
    );
    container.scrollTo({ top: targetScrollTop, behavior: "smooth" });
  }, [anchorNode]);

  useEffect(() => {
    const container = containerRef.current;

    if (!container || !anchorNode) {
      setIsAnchorVisible(true);
      setIsScrolledPastAnchor(false);
      return;
    }

    const observer = new IntersectionObserver(
      ([entry]) => {
        const containerRect = container.getBoundingClientRect();
        const anchorRect = anchorNode.getBoundingClientRect();

        setIsAnchorVisible(entry.isIntersecting);
        setIsScrolledPastAnchor(anchorRect.top < containerRect.top);
      },
      { root: container, threshold: 0.1 },
    );

    observer.observe(anchorNode);

    return () => observer.disconnect();
  }, [anchorNode]);

  return {
    containerRef,
    isAnchorVisible,
    isScrolledPastAnchor,
    scrollToAnchor,
    registerAnchor,
    anchorNode,
  };
}

export function useAutoScrollToAnchor({
  scrollFn,
  isVisible,
  anchorNode,
  deps = [],
}: {
  scrollFn: () => void;
  isVisible: boolean;
  anchorNode: HTMLDivElement | null;
  deps?: DependencyList;
}) {
  const hasInitialScrolledRef = useRef(false);
  const prevAnchorNodeRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!anchorNode || hasInitialScrolledRef.current) {
      return;
    }

    hasInitialScrolledRef.current = true;
    requestAnimationFrame(() => {
      scrollFn();
    });
  }, [anchorNode, scrollFn]);

  useEffect(() => {
    if (!anchorNode || prevAnchorNodeRef.current === anchorNode) {
      prevAnchorNodeRef.current = anchorNode;
      return;
    }

    prevAnchorNodeRef.current = anchorNode;

    requestAnimationFrame(() => {
      if (!isVisible) {
        scrollFn();
      }
    });
  }, [anchorNode, isVisible, scrollFn]);

  useEffect(() => {
    if (!anchorNode || isVisible) {
      return;
    }

    requestAnimationFrame(() => {
      scrollFn();
    });
  }, deps);
}
