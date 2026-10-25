import { Markets } from "@/components/bettor/Markets";
import { PageHead } from "@/components/ui/Scene3D";
import { featuredMarket, fetchMarkets, fetchTip } from "@/lib/api";

export default async function Page() {
  const [markets, tip] = await Promise.all([fetchMarkets(), fetchTip()]);
  const featured = featuredMarket(markets);
  return <section className="bettor-page"><PageHead index="S-01 / PUBLIC MARKETS" title="The ledger is open." kind="towers" yes={featured?.odds[0]?.probability ?? null} no={featured?.odds[1]?.probability ?? null} /><Markets initialMarkets={markets} initialTip={tip} /></section>;
}
