import Link from "next/link";
import type { ComponentProps } from "react";

type Variant = "primary" | "ghost";

const base =
  "inline-flex items-center gap-2 rounded-[var(--radius-2)] px-5 py-3 t-4 font-medium transition-colors focus-visible:outline-2";
const variants: Record<Variant, string> = {
  // --accent is reserved for the primary CTA (docs/DESIGN.md §2).
  primary: "bg-[var(--accent)] text-white hover:bg-[var(--accent-hover)]",
  ghost: "border border-[var(--border)] text-[var(--ice)] hover:border-[var(--ice-muted)]",
};

export function buttonClass(variant: Variant = "primary"): string {
  return `${base} ${variants[variant]}`;
}

export function Button({ variant = "primary", className = "", ...props }: ComponentProps<"button"> & { variant?: Variant }) {
  return <button className={`${buttonClass(variant)} ${className}`} {...props} />;
}

export function ButtonLink({ variant = "primary", className = "", ...props }: ComponentProps<typeof Link> & { variant?: Variant }) {
  return <Link className={`${buttonClass(variant)} ${className}`} {...props} />;
}
