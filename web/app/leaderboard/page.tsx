import { LeaderboardTable } from "@/components/persona/LeaderboardTable";
import { fetchLeaderboard } from "@/lib/persona";

export default async function Page() {
  return <section className="bettor-page"><p className="section-index">S-09 / LEADERBOARD</p><h1 className="t-2 my-6">Leaderboard.</h1>
    <p className="t-4 muted mb-6">Thawed Personas only. Every figure is recomputable from the Persona&apos;s public Thaw Report.</p>
    <LeaderboardTable entries={await fetchLeaderboard()} /></section>;
}
