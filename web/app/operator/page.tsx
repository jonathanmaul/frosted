import { OperatorStatus } from "@/components/bettor/OperatorStatus";
import { operatorRows } from "@/lib/operator";

export default async function Page() {
  return <OperatorStatus initialRows={await operatorRows()} />;
}
