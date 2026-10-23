import { OperatorStatus } from "@/components/bettor/OperatorStatus";
import { fetchMarkets } from "@/lib/api";
import { fetchPayout } from "@/lib/bettor";

export default async function Page() {
  const markets = await fetchMarkets();
  const rows = markets ? await Promise.all(markets.map(async market => ({ market, payout: await fetchPayout(market.market_id) }))) : null;
  return <OperatorStatus initialRows={rows} />;
}
