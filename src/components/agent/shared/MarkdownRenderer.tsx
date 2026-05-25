import React from "react";
import ReactMarkdown from "react-markdown";

interface MarkdownRendererProps {
  content: string;
}

export function MarkdownRenderer({ content }: MarkdownRendererProps) {
  return (
    <div
      className="prose prose-sm max-w-none"
      style={{ color: "var(--color-text-secondary)" }}
    >
      <ReactMarkdown
        components={{
          h1: ({ children }) => (
            <h1
              className="text-xl font-bold mb-2"
              style={{ color: "var(--color-text)" }}
            >
              {children}
            </h1>
          ),
          h2: ({ children }) => (
            <h2
              className="text-lg font-semibold mb-2"
              style={{ color: "var(--color-text)" }}
            >
              {children}
            </h2>
          ),
          h3: ({ children }) => (
            <h3
              className="text-base font-medium mb-2"
              style={{ color: "var(--color-text)" }}
            >
              {children}
            </h3>
          ),
          p: ({ children }) => (
            <p className="text-sm mb-2" style={{ color: "var(--color-text-secondary)" }}>
              {children}
            </p>
          ),
          ul: ({ children }) => (
            <ul
              className="list-disc list-inside space-y-1 mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              {children}
            </ul>
          ),
          ol: ({ children }) => (
            <ol
              className="list-decimal list-inside space-y-1 mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              {children}
            </ol>
          ),
          li: ({ children }) => (
            <li className="text-sm">{children}</li>
          ),
          code: ({ className, children }) => {
            const isInline = !className;
            if (isInline) {
              return (
                <code
                  className="px-1 py-0.5 rounded text-xs"
                  style={{
                    backgroundColor: "var(--color-surface-subtle)",
                    color: "var(--color-primary-text)",
                  }}
                >
                  {children}
                </code>
              );
            }
            return (
              <code className="text-xs font-mono" style={{ color: "var(--color-text-secondary)" }}>
                {children}
              </code>
            );
          },
          pre: ({ children }) => (
            <pre
              className="p-3 rounded-md my-2 overflow-x-auto"
              style={{
                backgroundColor: "var(--color-surface-subtle)",
                border: "1px solid var(--color-border)",
              }}
            >
              {children}
            </pre>
          ),
          strong: ({ children }) => (
            <strong className="font-semibold" style={{ color: "var(--color-text)" }}>
              {children}
            </strong>
          ),
          a: ({ href, children }) => (
            <a
              href={href}
              target="_blank"
              rel="noopener noreferrer"
              className="underline"
              style={{ color: "var(--color-primary-text)" }}
            >
              {children}
            </a>
          ),
          table: ({ children }) => (
            <div className="overflow-x-auto my-2">
              <table
                className="text-xs border-collapse"
                style={{ border: "1px solid var(--color-border)" }}
              >
                {children}
              </table>
            </div>
          ),
          th: ({ children }) => (
            <th
              className="px-2 py-1 text-left font-medium"
              style={{
                backgroundColor: "var(--color-surface-subtle)",
                border: "1px solid var(--color-border)",
                color: "var(--color-text)",
              }}
            >
              {children}
            </th>
          ),
          td: ({ children }) => (
            <td
              className="px-2 py-1"
              style={{
                border: "1px solid var(--color-border)",
                color: "var(--color-text-secondary)",
              }}
            >
              {children}
            </td>
          ),
          blockquote: ({ children }) => (
            <blockquote
              className="pl-3 my-2"
              style={{ borderLeft: "3px solid var(--color-border)" }}
            >
              {children}
            </blockquote>
          ),
          hr: () => (
            <hr className="my-3" style={{ border: "1px solid var(--color-border)" }} />
          ),
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
