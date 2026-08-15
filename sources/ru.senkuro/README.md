# Senkuro

Aidoku source for [senkuro.me](https://senkuro.me).

## Authentication

Senkuro protects chapter reading behind an account. Use the source settings
to open the real Senkuro website in Aidoku's web login flow, complete the
site's own sign-in process, and close the web view when the login is accepted.
On some installations, Aidoku may need to be restarted before the source can use
the newly saved login session.

The source replays the complete first-party cookie map saved by Aidoku. Native
OAuth is not configured because the provider currently rejects Aidoku's custom
callback URI; the web login flow uses Senkuro's registered website callback
instead.

## API

The source uses the public GraphQL endpoint at
`https://api.senkuro.me/graphql` for search, manga details, chapters, and
reader pages. Requests include the Senkuro origin and referer headers. If the
API returns an HTTP error, check the network route and VPN exit region first;
access may depend on the connection's region.

## Home and listings

The Home screen contains latest updates, daily popular titles, and new titles.
Recommendations are added when the API returns them. The corresponding
`ListingProvider` exposes paginated latest-update and new-title listings, plus
finite popular and recommendation listings.
