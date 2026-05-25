import React from "react";

interface JsonHighlightProps {
  data: unknown;
  maxHeight?: number;
}

export function JsonHighlight({ data, maxHeight = 400 }: JsonHighlightProps) {
  const formatJson = (obj: unknown, indent: number = 0): React.ReactNode[] => {
    const spaces = "  ".repeat(indent);
    const nodes: React.ReactNode[] = [];

    if (obj === null) {
      nodes.push(
        <span key={`null-${indent}`} style={{ color: "var(--color-text-muted)" }}>
          null
        </span>
      );
    } else if (obj === undefined) {
      nodes.push(
        <span key={`undefined-${indent}`} style={{ color: "var(--color-text-muted)" }}>
          undefined
        </span>
      );
    } else if (typeof obj === "boolean") {
      nodes.push(
        <span key={`bool-${indent}`} style={{ color: "var(--color-primary-text)" }}>
          {obj.toString()}
        </span>
      );
    } else if (typeof obj === "number") {
      nodes.push(
        <span key={`num-${indent}`} style={{ color: "var(--color-success)" }}>
          {obj}
        </span>
      );
    } else if (typeof obj === "string") {
      nodes.push(
        <span key={`str-${indent}`} style={{ color: "var(--color-warn-text)" }}>
          &quot;{obj}&quot;
        </span>
      );
    } else if (Array.isArray(obj)) {
      if (obj.length === 0) {
        nodes.push(
          <span key={`arr-empty-${indent}`} style={{ color: "var(--color-text-muted)" }}>
            []
          </span>
        );
      } else {
        nodes.push(
          <span key={`arr-open-${indent}`} style={{ color: "var(--color-text-muted)" }}>
            [
          </span>
        );
        nodes.push(<br key={`br-open-${indent}`} />);
        obj.forEach((item, index) => {
          nodes.push(
            <span key={`arr-item-indent-${indent}-${index}`}>&nbsp;&nbsp;{spaces}</span>
          );
          nodes.push(...formatJson(item, indent + 1));
          if (index < obj.length - 1) {
            nodes.push(
              <span key={`arr-comma-${indent}-${index}`} style={{ color: "var(--color-text-muted)" }}>
                ,
              </span>
            );
          }
          nodes.push(<br key={`br-${indent}-${index}`} />);
        });
        nodes.push(<span key={`arr-close-indent-${indent}`}>{spaces}</span>);
        nodes.push(
          <span key={`arr-close-${indent}`} style={{ color: "var(--color-text-muted)" }}>
            ]
          </span>
        );
      }
    } else if (typeof obj === "object") {
      const entries = Object.entries(obj as Record<string, unknown>);
      if (entries.length === 0) {
        nodes.push(
          <span key={`obj-empty-${indent}`} style={{ color: "var(--color-text-muted)" }}>
            {'{}'}
          </span>
        );
      } else {
        nodes.push(
          <span key={`obj-open-${indent}`} style={{ color: "var(--color-text-muted)" }}>
            {'{'}
          </span>
        );
        nodes.push(<br key={`br-obj-open-${indent}`} />);
        entries.forEach(([key, value], index) => {
          nodes.push(
            <span key={`obj-key-indent-${indent}-${index}`}>&nbsp;&nbsp;{spaces}</span>
          );
          nodes.push(
            <span key={`obj-key-${indent}-${index}`} style={{ color: "var(--color-primary-text)" }}>
              &quot;{key}&quot;
            </span>
          );
          nodes.push(
            <span key={`obj-colon-${indent}-${index}`} style={{ color: "var(--color-text-muted)" }}>
              :{" "}
            </span>
          );
          nodes.push(...formatJson(value, indent + 1));
          if (index < entries.length - 1) {
            nodes.push(
              <span key={`obj-comma-${indent}-${index}`} style={{ color: "var(--color-text-muted)" }}>
                ,
              </span>
            );
          }
          nodes.push(<br key={`br-obj-${indent}-${index}`} />);
        });
        nodes.push(<span key={`obj-close-indent-${indent}`}>{spaces}</span>);
        nodes.push(
          <span key={`obj-close-${indent}`} style={{ color: "var(--color-text-muted)" }}>
            {'}'}
          </span>
        );
      }
    }

    return nodes;
  };

  return (
    <pre
      className="p-3 rounded-md overflow-auto font-mono text-xs leading-relaxed"
      style={{
        backgroundColor: "var(--color-surface-subtle)",
        border: "1px solid var(--color-border)",
        maxHeight: `${maxHeight}px`,
        margin: 0,
      }}
    >
      <code>{formatJson(data)}</code>
    </pre>
  );
}
