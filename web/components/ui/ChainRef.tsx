import { explorerBlockUrl, explorerTxUrl } from "@/lib/explorer";

// A transaction id with a public-explorer link when the network has one.
export function TxRef({ txid, className = "ledger-hash num t-5" }: { txid: string; className?: string }) {
  const url = explorerTxUrl(txid);
  return <span className="chain-ref">
    <span className={className}>{txid}</span>
    {url && <> <a className="text-link t-5" href={url} target="_blank" rel="noopener noreferrer">View on explorer</a></>}
  </span>;
}

// A block height, linked to the explorer when available.
export function BlockRef({ height }: { height: number }) {
  const url = explorerBlockUrl(height);
  const label = height.toLocaleString("en-US");
  return url ? <a className="text-link num" href={url} target="_blank" rel="noopener noreferrer" aria-label={`Block ${label} on the explorer`}>{label}</a> : <span className="num">{label}</span>;
}
