import React from "react";
import { motion } from "framer-motion";

interface StatusDotProps {
  status: "idle" | "running" | "success" | "error";
  size?: "sm" | "md" | "lg";
}

export function StatusDot({ status, size = "md" }: StatusDotProps) {
  const sizeClasses = {
    sm: "w-2 h-2",
    md: "w-3 h-3",
    lg: "w-4 h-4",
  };

  const colors = {
    idle: "var(--color-text-muted)",
    running: "var(--color-primary)",
    success: "var(--color-success)",
    error: "var(--color-error)",
  };

  const animations = {
    idle: {},
    running: {
      scale: [1, 1.2, 1],
      transition: { duration: 1, repeat: Infinity },
    },
    success: {},
    error: {},
  };

  return (
    <motion.div
      className={`rounded-full ${sizeClasses[size]}`}
      style={{ backgroundColor: colors[status] }}
      animate={animations[status]}
    />
  );
}
