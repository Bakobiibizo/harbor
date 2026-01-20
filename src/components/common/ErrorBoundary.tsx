import { Component, ErrorInfo, ReactNode } from "react";

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    console.error("[Harbor] Uncaught error:", error, errorInfo);
  }

  handleReload = (): void => {
    window.location.reload();
  };

  handleReset = (): void => {
    this.setState({ hasError: false, error: null });
  };

  render(): ReactNode {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div className="min-h-screen bg-[var(--harbor-bg-primary)] flex items-center justify-center p-4">
          <div className="bg-[var(--harbor-bg-elevated)] rounded-lg p-6 max-w-md w-full border border-[var(--harbor-border-subtle)]">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-10 h-10 rounded-full bg-red-500/20 flex items-center justify-center">
                <svg
                  className="w-5 h-5 text-red-500"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
                  />
                </svg>
              </div>
              <div>
                <h2 className="text-lg font-semibold text-[var(--harbor-text-primary)]">
                  Something went wrong
                </h2>
                <p className="text-sm text-[var(--harbor-text-secondary)]">
                  An unexpected error occurred
                </p>
              </div>
            </div>

            {this.state.error && (
              <div className="bg-[var(--harbor-surface-1)] rounded p-3 mb-4 overflow-auto max-h-32">
                <code className="text-xs text-[var(--harbor-text-tertiary)] font-mono">
                  {this.state.error.message}
                </code>
              </div>
            )}

            <div className="flex gap-3">
              <button
                onClick={this.handleReset}
                className="flex-1 px-4 py-2 bg-[var(--harbor-surface-2)] text-[var(--harbor-text-primary)] rounded-lg hover:bg-[var(--harbor-surface-1)] transition-colors"
              >
                Try Again
              </button>
              <button
                onClick={this.handleReload}
                className="flex-1 px-4 py-2 bg-[var(--harbor-primary)] text-white rounded-lg hover:opacity-90 transition-opacity"
              >
                Reload App
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

export default ErrorBoundary;
