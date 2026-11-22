#!/usr/bin/env bash
# Renders the link preview cards, assets/og/*.png, from card.html.
#
# Run it by hand on macOS, so the font is SF as in the design; another system
# draws a different font, which is why CI never runs it. The PNGs it writes are
# committed. The first run may need `npx playwright@1.65.0 install chromium`.
set -euo pipefail

cd "$(dirname "$0")"

playwright=playwright@1.65.0
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# file|lang|kind|tone|question|word|pill
cards='open.es|es|status|green|¿Está abierto el Retiro?|Abierto|Sin alertas
open-with-restrictions.es|es|status|orange|¿Está abierto el Retiro?|Abierto|Con restricciones
open-stay-not-advised.es|es|status|orange|¿Está abierto el Retiro?|Abierto|No se recomienda permanecer
closure-expected.es|es|status|orange|¿Está abierto el Retiro?|Abierto|Cierre previsto por alerta
closed-by-alert.es|es|status|red|¿Está abierto el Retiro?|Cerrado|Alerta meteorológica
closed-by-schedule.es|es|status|neutral|¿Está abierto el Retiro?|Cerrado|Por horario
unknown.es|es|unknown||¿Está abierto el Retiro?||
open.en|en|status|green|Is Retiro Park open?|Open|No alerts
open-with-restrictions.en|en|status|orange|Is Retiro Park open?|Open|Restrictions in place
open-stay-not-advised.en|en|status|orange|Is Retiro Park open?|Open|Staying is not advised
closure-expected.en|en|status|orange|Is Retiro Park open?|Open|Closure expected due to an alert
closed-by-alert.en|en|status|red|Is Retiro Park open?|Closed|Weather alert
closed-by-schedule.en|en|status|neutral|Is Retiro Park open?|Closed|Outside opening hours
unknown.en|en|unknown||Is Retiro Park open?||'

while IFS='|' read -r file lang kind tone question word pill; do
    # No text holds a #, so it delimits the substitutions.
    LC_ALL=C sed \
        -e "s#{{lang}}#$lang#g" \
        -e "s#{{kind}}#$kind#g" \
        -e "s#{{tone}}#$tone#g" \
        -e "s#{{question}}#$question#g" \
        -e "s#{{word}}#$word#g" \
        -e "s#{{pill}}#$pill#g" \
        card.html >"$work/$file.html"
    npx --yes "$playwright" screenshot --viewport-size "1200, 630" \
        "file://$work/$file.html" "./$file.png"
done <<<"$cards"
