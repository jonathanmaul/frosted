import { ThawReportView } from "@/components/persona/ThawReportView";
import { MonoNumber } from "@/components/ui/MonoNumber";
import { fetchMarketLabels, fetchPersona, fetchThawReport, hasBadge, BADGE_MIN_BETS } from "@/lib/persona";

export default async function Page({ params }: { params: Promise<{ persona_id: string }> }) {
  const { persona_id } = await params;
  const persona = await fetchPersona(persona_id);
  const head = <p className="section-index">S-08 / PERSONA PROFILE</p>;
  if (!persona) return <section className="bettor-page">{head}<h1 className="t-2 my-6">Persona unavailable.</h1><p className="t-4 muted">No Persona with this ID is registered, or the public API is unreachable.</p></section>;
  const report = persona.status === "THAWED" ? await fetchThawReport(persona_id) : null;
  if (!report) return <section className="bettor-page">{head}<h1 className="t-2 my-6">Persona <MonoNumber>{persona_id.slice(0, 8)}</MonoNumber>.</h1>
    <p className="t-4 muted">Registered at block <MonoNumber>{persona.registered_height.toLocaleString("en-US")}</MonoNumber>. {persona.status === "THAWED" ? "The report is unavailable right now." : "This Persona has not thawed. Its bets stay private."}</p></section>;
  const markets = await fetchMarketLabels(report.bets.map((b) => b.market_id));
  return <section className="bettor-page">{head}<h1 className="t-2 my-6">{report.alias}.</h1>
    <p className="t-4 muted">Persona <MonoNumber>{persona_id.slice(0, 8)}</MonoNumber> · registered at block <MonoNumber>{report.registered_height.toLocaleString("en-US")}</MonoNumber>
      {hasBadge(report.stats.settled) && <> · <span className="badge-10">{BADGE_MIN_BETS}+ settled bets</span></>}</p>
    <div className="ledger-panel"><ThawReportView report={report} markets={markets} /></div></section>;
}
