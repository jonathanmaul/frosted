import { Landing } from "@/components/landing/Landing";
import { Markets } from "@/components/bettor/Markets";
import { fetchMarkets, fetchTip } from "@/lib/api";

export default async function Page() {
  const [markets, tip] = await Promise.all([fetchMarkets(), fetchTip()]);
  return <Landing markets={<Markets initialMarkets={markets} initialTip={tip} preview={3} />} />;
}
