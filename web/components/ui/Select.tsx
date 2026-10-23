"use client";

// Custom listbox select (docs/DESIGN.md: flat, hairlines, radius <= 4px, no native browser popup).
// WAI-ARIA select-only combobox pattern: focus stays on the button; the highlighted option is
// exposed through aria-activedescendant.
import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";

export type SelectOption = { value: string; label: string };

type Props = {
  label: string;
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
  id?: string;
  className?: string;
};

export function Select({ label, value, options, onChange, id, className = "" }: Props) {
  const auto = useId();
  const base = id ?? auto;
  const labelId = `${base}-label`, listId = `${base}-list`;
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const root = useRef<HTMLDivElement>(null);
  const typed = useRef({ text: "", at: 0 });
  const selected = Math.max(0, options.findIndex((o) => o.value === value));

  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => { if (!root.current?.contains(e.target as Node)) setOpen(false); };
    document.addEventListener("pointerdown", away);
    return () => document.removeEventListener("pointerdown", away);
  }, [open]);
  // Keep the highlighted option visible in a long list.
  useEffect(() => {
    if (open) document.getElementById(`${base}-opt-${active}`)?.scrollIntoView?.({ block: "nearest" });
  }, [open, active, base]);

  const show = () => { setActive(selected); setOpen(true); };
  const choose = (index: number) => { const o = options[index]; if (o) onChange(o.value); setOpen(false); };
  const move = (index: number) => setActive(Math.min(options.length - 1, Math.max(0, index)));

  function onKeyDown(e: KeyboardEvent<HTMLButtonElement>) {
    switch (e.key) {
      case "ArrowDown": e.preventDefault(); if (open) move(active + 1); else show(); break;
      case "ArrowUp": e.preventDefault(); if (open) move(active - 1); else show(); break;
      case "Home": if (open) { e.preventDefault(); move(0); } break;
      case "End": if (open) { e.preventDefault(); move(options.length - 1); } break;
      case "Enter": case " ":
        e.preventDefault(); if (open) choose(active); else show(); break;
      case "Escape": if (open) { e.preventDefault(); setOpen(false); } break;
      case "Tab": setOpen(false); break;
      default:
        // Typeahead: letters typed within 600 ms narrow to the next matching label.
        if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
          const now = Date.now(), t = typed.current;
          t.text = now - t.at > 600 ? e.key.toLowerCase() : t.text + e.key.toLowerCase();
          t.at = now;
          const from = t.text.length === 1 ? (open ? active : selected) + 1 : (open ? active : selected);
          const order = [...options.keys()].map((i) => (from + i) % options.length);
          const hit = order.find((i) => options[i].label.toLowerCase().startsWith(t.text));
          if (hit !== undefined) { if (!open) setOpen(true); setActive(hit); }
        }
    }
  }

  return <div className={`ui-select ${className}`} ref={root}>
    <span id={labelId} className="ui-select-label">{label}</span>
    <button type="button" id={base} role="combobox" className="ledger-input ui-select-button" aria-haspopup="listbox"
      aria-expanded={open} aria-controls={listId} aria-labelledby={labelId} aria-activedescendant={open ? `${base}-opt-${active}` : undefined}
      onClick={() => (open ? setOpen(false) : show())} onKeyDown={onKeyDown}>
      <span>{options[selected]?.label ?? ""}</span>
      <span className="ui-select-caret num" aria-hidden="true">{open ? "−" : "+"}</span>
    </button>
    {open && <ul id={listId} role="listbox" aria-labelledby={labelId} className="ui-select-list" tabIndex={-1}>
      {options.map((o, i) => <li key={o.value} id={`${base}-opt-${i}`} role="option" aria-selected={i === selected} data-active={i === active || undefined}
        className="ui-select-option t-4" onPointerEnter={() => setActive(i)}
        // Keep focus on the button so the keyboard keeps working after a click.
        onPointerDown={(e) => e.preventDefault()} onClick={() => choose(i)}>{o.label}</li>)}
    </ul>}
  </div>;
}
