# open-parks

Is Retiro open? Madrid park alert status, served as SSR HTML and a JSON API.

## Development

The toolchain is pinned in `rust-toolchain.toml`; rustup installs it on first use.

```sh
cargo run
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

CI runs the last three on every push to `main` and on pull requests.

## API

`cargo run` serves on `127.0.0.1:3000`; set `OPEN_PARKS_ADDRESS` to listen
elsewhere.

`GET /api/v1/parks/retiro/status` returns Retiro's status as machine values;
clients localise them.

```json
{
  "code": "red_alert_restrictions",
  "verdict": "open_with_restrictions",
  "dates": "12/10/2026",
  "hours": "de 14:00 a 20:00 h",
  "reopening": null,
  "notes": "Cerradas las zonas infantiles, deportivas y de mayores. Suspendidos los eventos al aire libre."
}
```

- `code`: `open`, `incidents`, `yellow_alert`, `orange_alert`,
  `red_alert_closure`, `red_alert_restrictions`, `closed` or `unknown`.
- `verdict`: `open`, `open_with_restrictions`, `open_stay_not_advised`,
  `closure_forecast`, `closed` or `unknown`.
- `reopening`: `after_inspections`, `see_notes`, `unknown` or `null`.
- `dates`, `hours` and `notes`: SIGMA's free text, or `null` when blank.

For now the server reports a fixed status rather than the live layer.
