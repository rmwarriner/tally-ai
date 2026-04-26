import { Component, type ReactNode } from "react";

interface Props { children: ReactNode }
interface State { hasError: boolean }

export class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false };

  static getDerivedStateFromError(): State {
    return { hasError: true };
  }

  componentDidCatch(error: unknown): void {
    // Log only — no telemetry until Phase 2.
    console.error("ErrorBoundary caught:", error);
  }

  render(): ReactNode {
    if (this.state.hasError) {
      return (
        <div role="alert" aria-live="assertive" className="error-boundary">
          <p>Something went wrong.</p>
          <button type="button" onClick={() => window.location.reload()}>
            Get help
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
