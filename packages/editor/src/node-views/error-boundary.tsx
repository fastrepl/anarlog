import type { NodeViewComponentProps } from "@handlewithcare/react-prosemirror";
import * as stylex from "@stylexjs/stylex";
import {
  Component,
  createElement,
  forwardRef,
  type ComponentType,
  type CSSProperties,
  type ErrorInfo,
  type ForwardedRef,
  type ReactNode,
} from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { cn } from "@anlg/utils";

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
      const { className, style, ...attrs } = this.props.fallbackAttrs;
      const fallbackStyles = stylex.props(styles.fallback);
      return createElement(
        this.props.fallbackTag,
        {
          ...attrs,
          ...fallbackStyles,
          ref: this.props.forwardedRef,
          contentEditable: false,
          suppressContentEditableWarning: true,
          "data-node-view-error": this.props.name,
          className: cn([
            fallbackStyles.className,
            typeof className === "string" ? className : "",
          ]),
          style: {
            ...fallbackStyles.style,
            ...(style as CSSProperties | undefined),
          },
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

const styles = stylex.create({
  fallback: {
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    paddingBlock: "0.25rem",
    paddingInline: "0.5rem",
  },
});
