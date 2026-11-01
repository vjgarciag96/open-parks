# Open-Meteo fixtures

Responses from Open-Meteo's `/v1/forecast` endpoint, used as golden inputs for
`open_meteo::decode` and served by the fake Open-Meteo in
`tests/integration/open_meteo_client.rs`.

## Provenance

Captured from the live API on 2026-11-02:

- `retiro.json`: Retiro's forecast, as returned for the client's request.
- `error_body.json`: the body returned (with status 400) for the same request
  with an invalid `forecast_hours`.
- `dst_fall_back.json`: Retiro's forecast for 2026-10-25, when the clocks went
  back, with `start_date` and `end_date` in place of `forecast_days`. 02:00
  comes twice. It has 24 hours from 01:00, not 25 from midnight.

Every rain chance was 0 on the capture date, so the rest are hand-edited
copies of `retiro.json`:

- `rain_chances.json`: rain chances that differ from hour to hour.
- `nulls.json`: `null` temperature, weather code and day/night at the first
  timestamp, and `null` rain chance and gusts at the second.
- `mismatched_lengths.json`: the last gust removed.
- `extra_value.json`: a rain chance appended after the last timestamp's.
- `is_day_2.json`: an `is_day` of 2 at the fourth timestamp.
- `malformed.json`: `retiro.json` cut off partway through.

`dst_spring_forward.json` is `dst_fall_back.json` with its timestamps replaced
by 24 hours from midnight on 2027-03-28, when the clocks go forward, so 02:00
is skipped. It was made before that date could be fetched.

## Caveats

`dst_spring_forward.json` shows how timestamps cross the change, not what
Open-Meteo sends that day: where its window starts and how many hours it has
are unknown. Replace it with a capture once the date can be fetched.

Each file keeps the full response, including the header fields that `decode`
ignores, so it reads exactly like what Open-Meteo sends.
