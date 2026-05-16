import { Component, type ErrorInfo, type ReactNode } from "react";

type EditorErrorBoundaryProps = {
  children: ReactNode;
};

type EditorErrorBoundaryState = {
  hasError: boolean;
};

export class EditorErrorBoundary extends Component<
  EditorErrorBoundaryProps,
  EditorErrorBoundaryState
> {
  constructor(props: EditorErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(): EditorErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Editor render failed", error, info);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div
          role="alert"
          className="rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2 text-sm text-neutral-600"
        >
          The editor hit an error. Your recording is still running.
        </div>
      );
    }

    return this.props.children;
  }
}
