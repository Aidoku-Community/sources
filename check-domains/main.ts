import { dirname, join } from "node:path";

import { checkDomainAlive, type AliveFailed } from "./domain-alive.ts";
import { checkDomainHttp, type DomainOk } from "./domain-redirect.ts";
import pLimit from "p-limit";
import { retryAsync } from "ts-retry";
import { readdir } from "node:fs/promises";
import process from "node:process";

const sources = join(dirname(import.meta.dirname ?? ""), "sources");

async function checkSource(
  name: string
): Promise<AliveFailed | DomainOk | void> {
  const meta = JSON.parse(
    await Deno.readTextFile(join(sources, name, "res/source.json")).catch(
      (err) => {
        console.warn(`Read file ${name}/res/source.json failed: ${err}`);
        return Promise.reject(err);
      }
    )
  ) as {
    info: {
      url?: string;
      urls?: string[];
    };
  };

  const urls = meta.info.urls ?? [meta.info.url!];

  // check first domain alive?
  for (const url of urls.slice(0, 1)) {
    const output = await retryAsync(
      async () => {
        const look = await checkDomainAlive(url);
        if (!look.alive) return look;

        const changed = await checkDomainHttp(url);
        if (changed) return changed;
      },
      { maxTry: 3 }
    );
    if (output) return output;
  }
}

const listSources = await readdir(sources);
const limit = pLimit(10);

console.log("Total size: %d", listSources.length);

const excludes = process.env.EXCLUDE?.split(",").filter(Boolean);

const sourcesDomainChanged: Map<string, [string, string]> = new Map();
const sourcesDomainDeiced: Map<string, string[]> = new Map();

await Promise.all(
  listSources.map((name) =>
    limit(async () => {
      if (excludes?.includes(name)) {
        console.info("Skip %s", name);

        return;
      }

      try {
        const output = await checkSource(name);
        if (output) {
          if (output.alive) {
            sourcesDomainChanged.set(name, [output.from, output.location]);
          } else {
            sourcesDomainDeiced.set(name, output.domains);
          }
        }
      } catch (error) {
        console.log(error);
      }
    })
  )
);

const outfile = await Deno.open(
  join(dirname(import.meta.dirname ?? ""), "check-domain-output.json"),
  { write: true, create: true }
);
await outfile.write(
  new TextEncoder().encode(
    JSON.stringify(
      {
        deiced: Object.fromEntries(sourcesDomainDeiced),
        changed: Object.fromEntries(sourcesDomainChanged),
      },
      null,
      2
    )
  )
);

console.log(`Ok Report:
    [+] ${sourcesDomainChanged.size} source changed domain
    [+] ${sourcesDomainDeiced.size} source deiced domain
`);
