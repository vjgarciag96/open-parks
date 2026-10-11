# open-parks

Is Retiro open? Madrid park alert status, served as SSR HTML and a JSON API.

## Development

The toolchain is pinned in `rust-toolchain.toml`, and the `Dockerfile`'s base
image pins the same version; rustup installs it on first use.

```sh
cargo run
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

CI runs the last three on every push to `main` and on pull requests.

## Container

```sh
docker build -t open-parks .
docker run -p 127.0.0.1:3000:3000 open-parks
```

The image holds only the static binary and listens on port 3000. CI builds it
on every pull request and publishes `ghcr.io/vjgarciag96/open-parks:<commit>`
on every push to `main`.

## API

`cargo run` serves on `127.0.0.1:3000`; set `OPEN_PARKS_ADDRESS` to listen
elsewhere. The status comes from Madrid's SIGMA alerts layer; set
`OPEN_PARKS_SIGMA_QUERY_URL` to query another endpoint, such as a local fake.

`GET /api/v1/parks/retiro/status` returns Retiro's status as machine values;
clients localise them.

```json
{
  "code": "red_alert_restrictions",
  "verdict": "open_with_restrictions",
  "dates": "12/10/2026",
  "hours": "de 14:00 a 20:00 h",
  "reopening": null,
  "notes": "Cerradas las zonas infantiles, deportivas y de mayores. Suspendidos los eventos al aire libre.",
  "fetched_at": "2026-10-11T09:30:00.123456Z"
}
```

- `code`: `open`, `incidents`, `yellow_alert`, `orange_alert`,
  `red_alert_closure`, `red_alert_restrictions`, `closed` or `unknown`.
- `verdict`: `open`, `open_with_restrictions`, `open_stay_not_advised`,
  `closure_forecast`, `closed` or `unknown`.
- `reopening`: `after_inspections`, `see_notes`, `unknown` or `null`.
- `dates`, `hours` and `notes`: SIGMA's free text, or `null` when blank.
- `fetched_at`: when the status was fetched from SIGMA, in RFC 3339 (UTC).

Requests never query SIGMA. The status is fetched at startup and refreshed
every minute in the background, and requests are served the latest status
fetched. A failed refresh is logged and the previous status keeps being served;
refreshes stay at one a minute rather than backing off, since that is already
light on SIGMA and picks up its recovery soonest. Only when no fetch has
succeeded yet is the response an error: a `502 Bad Gateway` with
`{"error": "upstream_error"}`, or a `504 Gateway Timeout` with
`{"error": "upstream_timeout"}` when SIGMA didn't answer in time.
