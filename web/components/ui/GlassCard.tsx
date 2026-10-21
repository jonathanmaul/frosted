import type { ComponentProps } from "react";

/** Glass surface. Allowed ONLY for the Bet Builder panel (docs/DESIGN.md §5). */
export function GlassCard({ className = "", ...props }: ComponentProps<"div">) {
  return (
    <div
      className={`rounded-[var(--radius-2)] border border-[var(--border)] bg-[var(--surface)] backdrop-blur-md ${className}`}
      data-surface="glass"
      {...props}
    />
  );
}
