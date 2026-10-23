import { MarketDetail } from "@/components/bettor/MarketDetail";
import { fetchMarket, fetchOdds, fetchBets, fetchResolution, fetchPayout } from "@/lib/bettor";

export default async function Page({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const [market, odds, bets, resolution, payout] = await Promise.all([fetchMarket(id), fetchOdds(id), fetchBets(id), fetchResolution(id), fetchPayout(id)]);
  if (!market) return <section className="bettor-page"><p className="section-index">S-02 / MARKET</p><h1 className="t-2 my-6">Market unavailable.</h1><p className="t-4 muted">Check the market ID and public API connection.</p></section>;
  return <MarketDetail initialMarket={market} initialOdds={odds} initialBets={bets} initialResolution={resolution} initialPayout={payout} />;
}
