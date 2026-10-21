import { ScreenPlaceholder } from "@/components/ScreenPlaceholder";

export default async function Page({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  return <ScreenPlaceholder id="S-10" title="Verify" features="F-12" note={`${id}: not implemented yet.`} />;
}
