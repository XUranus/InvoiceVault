import React from "react";

interface JsonViewerProps {
  data: unknown;
  maxHeight?: number;
}

export function JsonViewer({ data, maxHeight = 300 }: JsonViewerProps) {
  const renderValue = (value: unknown, indent: number = 0): React.ReactNode => {
    if (value === null) {
      return (
        <span style={{ color: "var(--color-text-muted)" }}>null</span>
      );
    }

    if (value === undefined) {
      return (
        <span style={{ color: "var(--color-text-muted)" }}>undefined</span>
      );
    }

    if (typeof value === "boolean") {
      return (
        <span style={{ color: "var(--color-primary-text)" }}>
          {value.toString()}
        </span>
      );
    }

    if (typeof value === "number") {
      return (
        <span style={{ color: "var(--color-success)" }}>{value}</span>
      );
    }

    if (typeof value === "string") {
      return (
        <span style={{ color: "var(--color-warn-text)" }}>
          &quot;{value}&quot;
        </span>
      );
    }

    if (Array.isArray(value)) {
      if (value.length === 0) {
        return <span style={{ color: "var(--color-text-muted)" }}>[]</span>;
      }

      return (
        <div>
          <span style={{ color: "var(--color-text-muted)" }}>[</span>
          <div className="ml-4">
            {value.map((item, index) => (
              <div key={index}>
                {renderValue(item, indent + 1)}
                {index < value.length - 1 && (
                  <span style={{ color: "var(--color-text-muted)" }}>,</span>
                )}
              </div>
            ))}
          </div>
          <span style={{ color: "var(--color-text-muted)" }}>]</span>
        </div>
      );
    }

    if (typeof value === "object") {
      const entries = Object.entries(value as Record<string, unknown>);
      if (entries.length === 0) {
        return <span style={{ color: "var(--color-text-muted)" }}>{}</span>;
      }

      return (
        <div>
          <span style={{ color: "var(--color-text-muted)" }}>{'{'}</span>
          <div className="ml-4">
            {entries.map(([key, val], index) => (
              <div key={key}>
                <span style={{ color: "var(--color-primary-text)" }}>
                  &quot;{key}&quot;
                </span>
                <span style={{ color: "var(--color-text-muted)" }}>: </span>
                {renderValue(val, indent + 1)}
                {index < entries.length - 1 && (
                  <span style={{ color: "var(--color-text-muted)" }}>,</span>
                )}
              </div>
            ))}
          </div>
          <span style={{ color: "var(--color-text-muted)" }}>{'}'}</span>
        </div>
      );
    }

    return <span>{String(value)}</span>;
  };

  return (
    <div
      className="p-3 rounded-md overflow-auto font-mono text-xs"
      style={{
        backgroundColor: "var(--color-surface-subtle)",
        border: "1px solid var(--color-border)",
        maxHeight: `${maxHeight}px`,
      }}
    >
      {renderValue(data)}
    </div>
  );
}
