import React from "react";
import { AlertTriangle } from "lucide-react";

type Props = {
  children: React.ReactNode;
  onError?: (error: string) => void;
};

type State = {
  hasError: boolean;
  error: string | null;
};

export class ErrorBoundary extends React.Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error: error.message || String(error) };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error("ErrorBoundary caught:", error, info.componentStack);
    this.props.onError?.(error.message || String(error));
  }

  handleReset = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="error-boundary">
          <div className="error-boundary-card">
            <AlertTriangle size={48} className="error-boundary-icon" />
            <h3>页面发生错误</h3>
            <p className="muted">
              {this.state.error || "未知错误"}
            </p>
            <div className="error-boundary-actions">
              <button
                className="btn-primary"
                onClick={this.handleReset}
              >
                重试
              </button>
              <button
                className="btn-small"
                onClick={() => window.location.reload()}
              >
                刷新页面
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
