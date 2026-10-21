import { ScreenPlaceholder } from "@/components/ScreenPlaceholder";

export default async function Page({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  return <ScreenPlaceholder id="S-02 / S-03 / S-05" title="Market Detail, Bet Builder, Resolution & Payout" features="F-02, F-03, F-04, F-06, F-07, F-08, F-11" note={`${id}: not implemented yet.`} />;
}
