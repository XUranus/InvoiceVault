import React from "react";

interface JsonHighlightProps {
  data: unknown;
  maxHeight?: number;
}

function isCommaElement(node: React.ReactNode): boolean {
  return (
    React.isValidElement<{ children?: React.ReactNode }>(node) &&
    typeof node.props.children === "string" &&
    node.props.children.includes(",")
  );
}

export function JsonHighlight({ data, maxHeight = 400 }: JsonHighlightProps) {
  const keyRef = React.useRef(0);

  const formatJson = (obj: unknown, indent: number = 0): React.ReactNode[] => {
    const spaces = "  ".repeat(indent);
    const nodes: React.ReactNode[] = [];
    const k = () => `j${keyRef.current++}`;

    if (obj === null) {
      nodes.push(
        <span key={k()} style={{ color: "var(--color-text-muted)" }}>
          null
        </span>
      );
    } else if (obj === undefined) {
      nodes.push(
        <span key={k()} style={{ color: "var(--color-text-muted)" }}>
          undefined
        </span>
      );
    } else if (typeof obj === "boolean") {
      nodes.push(
        <span key={k()} style={{ color: "var(--color-primary-text)" }}>
          {obj.toString()}
        </span>
      );
    } else if (typeof obj === "number") {
      nodes.push(
        <span key={k()} style={{ color: "var(--color-success)" }}>
          {obj}
        </span>
      );
    } else if (typeof obj === "string") {
      nodes.push(
        <span key={k()} style={{ color: "var(--color-warn-text)" }}>
          &quot;{obj}&quot;
        </span>
      );
    } else if (Array.isArray(obj)) {
      if (obj.length === 0) {
        nodes.push(
          <span key={k()} style={{ color: "var(--color-text-muted)" }}>
            []
          </span>
        );
      } else {
        nodes.push(
          <span key={k()} style={{ color: "var(--color-text-muted)" }}>
            [
          </span>
        );
        nodes.push(<br key={k()} />);
        obj.forEach((item) => {
          nodes.push(
            <span key={k()}>&nbsp;&nbsp;{spaces}</span>
          );
          nodes.push(...formatJson(item, indent + 1));
          nodes.push(
            <span key={k()} style={{ color: "var(--color-text-muted)" }}>
              ,
            </span>
          );
          nodes.push(<br key={k()} />);
        });
        // Remove trailing comma+br from last item
        if (nodes.length >= 2) {
          const last = nodes[nodes.length - 1];
          const secondLast = nodes[nodes.length - 2];
          if (React.isValidElement(last) && last.type === "br" && isCommaElement(secondLast)) {
            nodes.length -= 2;
            nodes.push(<br key={k()} />);
          }
        }
        nodes.push(<span key={k()}>{spaces}</span>);
        nodes.push(
          <span key={k()} style={{ color: "var(--color-text-muted)" }}>
            ]
          </span>
        );
      }
    } else if (typeof obj === "object") {
      const entries = Object.entries(obj as Record<string, unknown>);
      if (entries.length === 0) {
        nodes.push(
          <span key={k()} style={{ color: "var(--color-text-muted)" }}>
            {'{}'}
          </span>
        );
      } else {
        nodes.push(
          <span key={k()} style={{ color: "var(--color-text-muted)" }}>
            {'{'}
          </span>
        );
        nodes.push(<br key={k()} />);
        entries.forEach(([key, value]) => {
          nodes.push(
            <span key={k()}>&nbsp;&nbsp;{spaces}</span>
          );
          nodes.push(
            <span key={k()} style={{ color: "var(--color-primary-text)" }}>
              &quot;{key}&quot;
            </span>
          );
          nodes.push(
            <span key={k()} style={{ color: "var(--color-text-muted)" }}>
              :{" "}
            </span>
          );
          nodes.push(...formatJson(value, indent + 1));
          nodes.push(
            <span key={k()} style={{ color: "var(--color-text-muted)" }}>
              ,
            </span>
          );
          nodes.push(<br key={k()} />);
        });
        // Remove trailing comma+br from last entry
        if (nodes.length >= 2) {
          const last = nodes[nodes.length - 1];
          const secondLast = nodes[nodes.length - 2];
          if (React.isValidElement(last) && last.type === "br" && isCommaElement(secondLast)) {
            nodes.length -= 2;
            nodes.push(<br key={k()} />);
          }
        }
        nodes.push(<span key={k()}>{spaces}</span>);
        nodes.push(
          <span key={k()} style={{ color: "var(--color-text-muted)" }}>
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
