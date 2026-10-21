type Props = { id: string; title: string; features: string; note?: string };

export function ScreenPlaceholder({ id, title, features, note }: Props) {
  return (
    <section className="space-y-2">
      <p className="text-sm text-slate-500">
        {id} · {features}
      </p>
      <h1 className="text-2xl font-semibold">{title}</h1>
      <p className="text-slate-600">{note ?? "Not implemented yet."}</p>
    </section>
  );
}
