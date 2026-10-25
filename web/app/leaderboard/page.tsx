import { LeaderboardTable } from "@/components/persona/LeaderboardTable";
import { PageHead } from "@/components/ui/Scene3D";
import { fetchLeaderboard } from "@/lib/persona";

export default async function Page() {
  return <section className="bettor-page"><PageHead index="S-09 / LEADERBOARD" title="Leaderboard." kind="towers" lede="Thawed Personas only. Every figure is recomputable from the Persona&apos;s public Thaw Report." />
    <LeaderboardTable entries={await fetchLeaderboard()} /></section>;
}
