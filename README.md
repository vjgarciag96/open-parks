# open-parks

Is Retiro open? Madrid park alert status, served as a web page
([retiroabierto.es](https://retiroabierto.es)) and a JSON API
([`/api/v1/parks/retiro/status`](https://retiroabierto.es/api/v1/parks/retiro/status)),
with an optional Telegram bot that notifies subscribers when the park closes or
reopens.

## Development

```sh
cargo run
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

To see the page in every state without a server, run
`cargo run --example preview` and open the `target/preview/index.html` it prints.

## Container

```sh
docker build -t open-parks .
docker run -p 127.0.0.1:3000:3000 open-parks
```

The image listens on port 3000 and has a writable `/data` directory for the
database.

## Configuration

| Variable | Purpose |
| --- | --- |
| `OPEN_PARKS_ADDRESS` | Address to listen on (default `127.0.0.1:3000`) |
| `OPEN_PARKS_PUBLIC_URL` | Origin the site is served at, for links that leave it |
| `OPEN_PARKS_SIGMA_QUERY_URL` | SIGMA endpoint to query, such as a local fake |
| `OPEN_PARKS_OPEN_METEO_URL` | Open-Meteo endpoint to query, such as a local fake |
| `OPEN_PARKS_DATABASE` | SQLite database path, needed by the Telegram bot |
| `OPEN_PARKS_TELEGRAM_BOT_TOKEN` | Runs the Telegram bot |
| `OPEN_PARKS_TELEGRAM_BOT_USERNAME` | The bot's username, without the `@`; the page links to it |

## Data source

The status is our interpretation of data from the Ayuntamiento de Madrid's
[SIGMA park alerts layer](https://sigma.madrid.es/hosted/rest/services/MEDIO_AMBIENTE/ALERTAS_PARQUES/MapServer/0)
(see also its [park closure status page](https://www.madrid.es/portal/site/munimadrid/menuitem.0c57021e0d1f6162c345c34571f1a5a0/?vgnextfmt=default&vgnextoid=51c3fb4f3f2cc710VgnVCM1000001d4a900aRCRD&vgnextchannel=3edd31d3b28fe410VgnVCM1000000b205a0aRCRD&vgnextlocale=es)),
reused under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/deed.es),
the licence of Madrid's open data [terms of use](https://datos.madrid.es/pages/condiciones-de-uso). This
project is not affiliated with or endorsed by the Ayuntamiento. If you reuse the
API's data, credit the Ayuntamiento de Madrid as its source too; `fetched_at`
gives the date of the data.
