// HTTPステータスとリダイレクトを確認する関数

import type { AliveFailed } from "./domain-alive.ts";

export interface DomainOk {
  alive: true;
  from: string;
  location: string;
}
export async function checkDomainHttp(
  domain: string
): Promise<DomainOk | AliveFailed | void> {
  const url = domain.startsWith("http") ? domain : `https://${domain}`;

  try {
    const res = await fetch(url, {
      redirect: "manual", // do NOT auto redirect
      method: "GET",
    });

    // Redirect?
    const location = res.headers.get("location");

    if (location) {
      const objUrl = new URL(url);
      const objLocation = new URL(location, url);

      if (
        !checkDomainEqual(objUrl, objLocation, "protocol") ||
        !checkDomainEqual(objUrl, objLocation, "hostname") ||
        !checkDomainEqual(objUrl, objLocation, "port") ||
        // !checkDomainEqual(objUrl, objLocation, "pathname") ||
        !checkDomainEqual(objUrl, objLocation, "username") ||
        !checkDomainEqual(objUrl, objLocation, "password")
      ) {
        console.log("%s redirected to: %s", url, location);
        return {
          alive: true,
          from: url,
          location,
        };
      }
    }
    // deno-lint-ignore no-explicit-any
  } catch (err) {
    console.log("HTTP check failed ✗", err instanceof Error ? err.message : String(err));
    return { alive: false, err, domains: [url] };
  }
}

function checkDomainEqual(u1: URL, u2: URL, name: keyof URL): boolean {
  return u1[name] === u2[name];
}
