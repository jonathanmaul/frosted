import { ScreenPlaceholder } from "@/components/ScreenPlaceholder";

export default async function Page({ params }: { params: Promise<{ persona_id: string }> }) {
  const { persona_id } = await params;
  return <ScreenPlaceholder id="S-08" title="Persona Profile" features="F-09, F-10" note={`${persona_id}: not implemented yet.`} />;
}
