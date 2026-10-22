"use client";

// Radix UI primitives styled with docs/DESIGN.md tokens (no shadcn/ui).
import * as DialogPrimitive from "@radix-ui/react-dialog";
import * as TabsPrimitive from "@radix-ui/react-tabs";
import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import type { ReactNode } from "react";

export function Dialog({ trigger, title, description, children }: { trigger: ReactNode; title: string; description?: string; children: ReactNode }) {
  return (
    <DialogPrimitive.Root>
      <DialogPrimitive.Trigger asChild>{trigger}</DialogPrimitive.Trigger>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 bg-[var(--bg)]/80" />
        <DialogPrimitive.Content className="fixed left-1/2 top-1/2 w-[min(560px,92vw)] -translate-x-1/2 -translate-y-1/2 rounded-[var(--radius-2)] border border-[var(--border)] bg-[var(--bg)] p-6">
          <DialogPrimitive.Title className="t-3 mb-4">{title}</DialogPrimitive.Title>
          {description && <DialogPrimitive.Description className="t-4 mb-6 text-[var(--ice-muted)]">{description}</DialogPrimitive.Description>}
          {children}
          <DialogPrimitive.Close className="num t-5 mt-6 text-[var(--ice-muted)] underline">Close</DialogPrimitive.Close>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}

export function Tabs({ tabs }: { tabs: { id: string; label: string; content: ReactNode }[] }) {
  return (
    <TabsPrimitive.Root defaultValue={tabs[0]?.id}>
      <TabsPrimitive.List className="flex gap-6 border-b border-[var(--border)]">
        {tabs.map((t) => (
          <TabsPrimitive.Trigger
            key={t.id}
            value={t.id}
            className="t-5 num -mb-px border-b border-transparent pb-2 uppercase tracking-wider text-[var(--ice-muted)] data-[state=active]:border-[var(--ice)] data-[state=active]:text-[var(--ice)]"
          >
            {t.label}
          </TabsPrimitive.Trigger>
        ))}
      </TabsPrimitive.List>
      {tabs.map((t) => (
        <TabsPrimitive.Content key={t.id} value={t.id} className="pt-4">
          {t.content}
        </TabsPrimitive.Content>
      ))}
    </TabsPrimitive.Root>
  );
}

export function Tooltip({ label, children }: { label: string; children: ReactNode }) {
  return (
    <TooltipPrimitive.Provider delayDuration={200}>
      <TooltipPrimitive.Root>
        <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
        <TooltipPrimitive.Portal>
          <TooltipPrimitive.Content className="t-5 rounded-[var(--radius-1)] border border-[var(--border)] bg-[var(--bg)] px-2 py-1">
            {label}
          </TooltipPrimitive.Content>
        </TooltipPrimitive.Portal>
      </TooltipPrimitive.Root>
    </TooltipPrimitive.Provider>
  );
}
