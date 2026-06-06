import type { NodeViewComponentProps } from "@handlewithcare/react-prosemirror";
import {
  Component,
  createElement,
  forwardRef,
  type ComponentType,
  type ErrorInfo,
  type ForwardedRef,
  type ReactNode,
} from "react";

type FallbackTag = "div" | "li" | "span";

type NodeViewErrorBoundaryProps = {
  children: ReactNode;
  fallbackAttrs: Record<string, unknown>;
  fallbackTag: FallbackTag;
  fallbackText: string;
  forwardedRef: ForwardedRef<HTMLElement>;
  name: string;
  resetKey: unknown;
};

type NodeViewErrorBoundaryState = {
  hasError: boolean;
};

class NodeViewErrorBoundary extends Component<
  NodeViewErrorBoundaryProps,
  NodeViewErrorBoundaryState
> {
  constructor(props: NodeViewErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(): NodeViewErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Editor node view render failed", {
      error,
      info,
      nodeView: this.props.name,
    });
  }

  componentDidUpdate(prevProps: NodeViewErrorBoundaryProps) {
    if (prevProps.resetKey === this.props.resetKey || !this.state.hasError) {
      return;
    }

    this.setState({ hasError: false });
  }

  render() {
    if (this.state.hasError) {
      const { className, ...attrs } = this.props.fallbackAttrs;
      return createElement(
        this.props.fallbackTag,
        {
          ...attrs,
          ref: this.props.forwardedRef,
          contentEditable: false,
          suppressContentEditableWarning: true,
          "data-node-view-error": this.props.name,
          className: [
            "rounded-md border border-border bg-muted px-2 py-1 text-sm text-muted-foreground",
            typeof className === "string" ? className : "",
          ]
            .filter(Boolean)
            .join(" "),
        },
        this.props.fallbackText,
      );
    }

    return this.props.children;
  }
}

function getFallbackAttrs(props: Record<string, unknown>) {
  const { children: _children, nodeProps: _nodeProps, ...attrs } = props;
  return attrs;
}

function getFallbackText(props: Partial<NodeViewComponentProps>) {
  const node = props.nodeProps?.node;
  const text = node?.textContent?.trim();
  if (text) {
    return text;
  }

  const attrs = node?.attrs ?? {};
  if (typeof attrs.name === "string" && attrs.name.trim()) {
    return attrs.name;
  }
  if (typeof attrs.url === "string" && attrs.url.trim()) {
    return attrs.url;
  }

  return "Unsupported content";
}

export function getNodeViewFallbackTag(name: string): FallbackTag {
  if (name === "taskItem") {
    return "li";
  }
  if (
    name === "attachment" ||
    name === "appLink" ||
    name.startsWith("mention")
  ) {
    return "span";
  }
  return "div";
}

export function getSafeNodePos(getPos: () => number | undefined) {
  try {
    const pos = getPos();
    return typeof pos === "number" && Number.isFinite(pos) ? pos : null;
  } catch {
    return null;
  }
}

export function withNodeViewErrorBoundary<E extends HTMLElement>(
  Component: ComponentType<any>,
  {
    fallbackTag,
    name,
  }: {
    fallbackTag?: FallbackTag;
    name: string;
  },
) {
  const Wrapped = forwardRef<E, any>((props, ref) => (
    <NodeViewErrorBoundary
      fallbackAttrs={getFallbackAttrs(props)}
      fallbackTag={fallbackTag ?? getNodeViewFallbackTag(name)}
      fallbackText={getFallbackText(props)}
      forwardedRef={ref as ForwardedRef<HTMLElement>}
      name={name}
      resetKey={props.nodeProps?.node}
    >
      <Component {...props} ref={ref} />
    </NodeViewErrorBoundary>
  ));

  Wrapped.displayName = `NodeViewErrorBoundary(${name})`;
  return Wrapped;
}
