# open-parks

Is Retiro open? Madrid park alert status, served as SSR HTML
([retiroabierto.duckdns.org](https://retiroabierto.duckdns.org)) and a JSON API
([`/api/v1/parks/retiro/status`](https://retiroabierto.duckdns.org/api/v1/parks/retiro/status)).

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

To see the page in every state, run `cargo run --example preview` and open
the `target/preview/index.html` it prints. It renders both languages for every
fixture in `tests/fixtures/sigma` that decodes, the most detailed status at a
few ages (including stale), and both error pages, with no server or SIGMA.

## Container

```sh
docker build -t open-parks .
docker run -p 127.0.0.1:3000:3000 open-parks
```

The image holds only the static binary and listens on port 3000. CI builds it
on every pull request and publishes `ghcr.io/vjgarciag96/open-parks:<commit>`
on every push to `main`.

## Serving

`cargo run` serves on `127.0.0.1:3000`; set `OPEN_PARKS_ADDRESS` to listen
elsewhere. The status comes from Madrid's SIGMA alerts layer; set
`OPEN_PARKS_SIGMA_QUERY_URL` to query another endpoint, such as a local fake.

## Page

`GET /es` and `GET /en` serve the status page in Spanish and English: a
server-rendered HTML page with no JavaScript and inline CSS. Each links to the
other language, and its footer credits the data source and its licence.

Under the verdict, the page lists whichever of the dates, hours, reopening
forecast and notes SIGMA gave, and says how long ago the status was fetched.
Once that is ten minutes or more (ten missed refreshes), it also warns that the
status may be out of date.

`GET /` redirects (`302 Found`) to the language the browser's `Accept-Language`
ranks highest, or to `/es` when it accepts neither. Each language has its own
URL so links and caches keep the language; the JSON API stays language-free.

## Data source

The status is our interpretation of data from the Ayuntamiento de Madrid's
[SIGMA park alerts layer](https://sigma.madrid.es/hosted/rest/services/MEDIO_AMBIENTE/ALERTAS_PARQUES/MapServer/0)
(see also its [park closure status page](https://www.madrid.es/portal/site/munimadrid/menuitem.0c57021e0d1f6162c345c34571f1a5a0/?vgnextfmt=default&vgnextoid=51c3fb4f3f2cc710VgnVCM1000001d4a900aRCRD&vgnextchannel=3edd31d3b28fe410VgnVCM1000000b205a0aRCRD&vgnextlocale=es)),
reused under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/deed.es),
the licence of Madrid's open data [terms of use](https://datos.madrid.es/pages/condiciones-de-uso). This
project is not affiliated with or endorsed by the Ayuntamiento. If you reuse the
API's data, credit the Ayuntamiento de Madrid as its source too; `fetched_at`
gives the date of the data.

## API

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
  "fetched_at": "2026-10-11T09:30:00Z"
}
```

- `code`: `open`, `incidents`, `yellow_alert`, `orange_alert`,
  `red_alert_closure`, `red_alert_restrictions`, `closed` or `unknown`.
- `verdict`: `open`, `open_with_restrictions`, `open_stay_not_advised`,
  `closure_forecast`, `closed` or `unknown`.
- `reopening`: `after_inspections`, `see_notes`, `unknown` or `null`.
- `dates`, `hours` and `notes`: SIGMA's free text, or `null` when blank.
- `fetched_at`: when the status was fetched from SIGMA, to the second, in
  RFC 3339 (UTC).

Requests never query SIGMA. The status is fetched at startup and refreshed
every minute in the background, and requests are served the latest status
fetched. A failed refresh is logged and the previous status keeps being served;
refreshes stay at one a minute rather than backing off, since that is already
light on SIGMA and picks up its recovery soonest. Only when no fetch has
succeeded yet is the response an error: a `502 Bad Gateway` with
`{"error": "upstream_error"}`, or a `504 Gateway Timeout` with
`{"error": "upstream_timeout"}` when SIGMA didn't answer in time.
