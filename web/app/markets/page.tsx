import { Markets } from "@/components/bettor/Markets";
import { fetchMarkets, fetchTip } from "@/lib/api";

export default async function Page() {
  const [markets, tip] = await Promise.all([fetchMarkets(), fetchTip()]);
  return <section className="bettor-page"><p className="section-index">S-01 / PUBLIC MARKETS</p><h1 className="t-2 my-6">The ledger is open.</h1><Markets initialMarkets={markets} initialTip={tip} /></section>;
}
