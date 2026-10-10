# SIGMA fixtures

Responses from the `ALERTAS_PARQUES` layer's `query` endpoint, used as golden
inputs for `sigma::decode`.

## Provenance

Captured from the live layer on 2026-10-10:

- `code_1_open.json`: Retiro's query, as returned.
- `no_features.json`: the same query for a park that doesn't exist.
- `error_body.json`: the body returned (with status 200) for an invalid
  `where` clause.

Every park reported code 1 on the capture date, so the rest are hand-edited
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

The dates, hours and notes in the hand-edited fixtures are invented. Real
formats for `FECHA_INCIDENCIA` ranges and `HORARIO_INCIDENCIA` are unknown, so
don't design their parsing from these files; capture a real response first and
add it here.

Each file keeps the full response, including the `fields` metadata that
`decode` ignores, so it reads exactly like what SIGMA sends.
