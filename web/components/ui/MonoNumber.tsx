import type { ComponentProps } from "react";

/** Every number (block, ZEC, odds, hash, percent) renders in IBM Plex Mono with tabular numerals. */
export function MonoNumber({ className = "", ...props }: ComponentProps<"span">) {
  return <span className={`num ${className}`} {...props} />;
}
