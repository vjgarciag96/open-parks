# SIGMA fixtures

Responses from the `ALERTAS_PARQUES` layer's `query` endpoint, used as golden
inputs for `sigma::decode` and served by the fake SIGMA in
`tests/integration/sigma_client.rs`.

## Provenance

Captured from the live layer on 2026-10-10:

- `code_1_open.json`: Retiro's query, as returned.
- `no_features.json`: the same query for a park that doesn't exist.
- `error_body.json`: the body returned (with status 200) for an invalid
  `where` clause.

Captured from the live layer on 2026-11-06:

- `code_1_open_with_hours.json`: Retiro's query, with incident hours under
  code 1.

Captured from the live layer on 2026-11-11, while the Ayuntamiento had closed
Retiro for wind:

- `code_6_live_closure.json`: Retiro's query, as returned.

Captured from the live layer on 2026-11-18, the evening Retiro closed under a
red alert:

- `code_7_live_closure_without_hours.json`: Retiro's query, as returned at
  19:53 Madrid time, with code 7 and the date but no hours.
- `code_5_live_forecast_without_hours.json`: the same response with code 5,
  as SIGMA returned it at 19:26, and the site still fetched at 19:38, while
  the park was still open. The
  attributes were recorded field by field at the time; the rest of the response
  is the 19:53 capture, identical to the 11/11 one. SIGMA switched to code 7
  between 19:38 and 19:44, before the 21:00 closure newspapers had reported.
  Neither code came with hours, so a date alone doesn't say when a closure
  starts.

Every park reported code 1 on the first capture date, so the rest are hand-edited
copies of the captures above:

- `code_2_incidents.json` to `code_7_closed.json`: one per alert code.
- `multiple_features.json`: a second, conflicting row appended.
- `unknown_codes.json`: alert and reopening codes SIGMA doesn't document.
- `null_alert_code.json`: `ALERTA_DESCRIPCION` set to `null`.
- `error_body_without_message.json`: `error_body.json` without its `message`.
- `blank_strings.json`: empty and whitespace-only free-text fields.
- `malformed.json`: `code_1_open.json` cut off partway through.
- `trailing_garbage.json`: `code_1_open.json` followed by an HTML error page.

## Caveats

The dates, hours and notes in the hand-edited fixtures are invented. The real
format for `FECHA_INCIDENCIA` ranges is unknown, and `HORARIO_INCIDENCIA` has
only been seen as `03:00 a 12:00`, `12:00 a 24:00` and `12:00-24:00`, so don't
design their parsing from these files; capture a real response first and add
it here.

Each file keeps the full response, including the `fields` metadata that
`decode` ignores, so it reads exactly like what SIGMA sends.
