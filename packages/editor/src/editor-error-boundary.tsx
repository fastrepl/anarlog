import * as stylex from "@stylexjs/stylex";
import { Component, Fragment, type ErrorInfo, type ReactNode } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

type EditorErrorBoundaryProps = {
  children: ReactNode;
  resetKey?: string;
};

type EditorErrorBoundaryState = {
  hasError: boolean;
  recoveryAttempts: number;
  recoveryKey: number;
};

const MAX_AUTO_RECOVERY_ATTEMPTS = 1;

export class EditorErrorBoundary extends Component<
  EditorErrorBoundaryProps,
  EditorErrorBoundaryState
> {
  constructor(props: EditorErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, recoveryAttempts: 0, recoveryKey: 0 };
  }

  static getDerivedStateFromError(): Partial<EditorErrorBoundaryState> {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Editor render failed", error, info);

    if (this.state.recoveryAttempts < MAX_AUTO_RECOVERY_ATTEMPTS) {
      this.setState((state) => ({
        hasError: false,
        recoveryAttempts: state.recoveryAttempts + 1,
        recoveryKey: state.recoveryKey + 1,
      }));
    }
  }

  componentDidUpdate(prevProps: EditorErrorBoundaryProps) {
    if (prevProps.resetKey === this.props.resetKey) {
      return;
    }

    this.setState((state) => ({
      hasError: false,
      recoveryAttempts: 0,
      recoveryKey: state.recoveryKey + 1,
    }));
  }

  private retry = () => {
    this.setState((state) => ({
      hasError: false,
      recoveryAttempts: 0,
      recoveryKey: state.recoveryKey + 1,
    }));
  };

  render() {
    if (this.state.hasError) {
      return (
        <div role="alert" {...stylex.props(styles.fallback)}>
          <span>
            The editor failed to render. Your recording is still running.
          </span>
          <button
            type="button"
            onClick={this.retry}
            {...stylex.props(styles.retry)}
          >
            Reload editor
          </button>
        </div>
      );
    }

    return (
      <Fragment key={this.state.recoveryKey}>{this.props.children}</Fragment>
    );
  }
}

const styles = stylex.create({
  fallback: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.75rem",
    justifyContent: "space-between",
    lineHeight: "1.25rem",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
  },
  retry: {
    backgroundColor: {
      default: colors.card,
      ":hover": colors.accent,
    },
    borderColor: colors.border,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.mutedForeground,
    flexShrink: 0,
    fontSize: "0.75rem",
    fontWeight: 500,
    lineHeight: "1rem",
    paddingBlock: "0.25rem",
    paddingInline: "0.5rem",
  },
});
