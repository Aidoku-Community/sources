// ドメインが生きているかどうかをDNSで確認する
// google dns?

import type { LookupAddress } from "node:dns";
import { lookup } from "node:dns/promises";

export interface AliveOk {
  alive: true;
  records: LookupAddress[];
}
export interface AliveFailed {
  alive: false;
  domains: string[];
  err: unknown;
}
export async function checkDomainAlive(
  url: string
): Promise<AliveOk | AliveFailed> {
  try {
    const result = await lookup(new URL(url).hostname, { all: true });

    // console.log("DNS Records:", result);
    console.log("Domain %s is alive ✓", url);

    return { alive: true, records: result };
  } catch (err) {
    console.log("DNS lookup failed ✗", err);
    return { alive: false, domains: [url], err };
  }
}
