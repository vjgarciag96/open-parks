#!/usr/bin/env bash
# Compares two renders of `cargo run --example preview`, screenshots each page
# that changed, before and after, and writes a PR comment showing them.
#
# Usage: page-preview.sh BEFORE_DIR AFTER_DIR OUT_DIR IMAGE_URL BASE_NAME
# Writes the screenshots and comment.md to OUT_DIR, with the screenshots
# referenced under IMAGE_URL, and prints how many pages changed.
set -eo pipefail

before=$1 after=$2 out=$3 image_url=$4 base_name=$5
# Each page takes six screenshots, so a change to every page stops at a few.
max_pages=6
# The workflow installs this version's browser.
playwright=playwright@1.65.0

mkdir -p "$out"

changed=()
for path in "$after"/*.html; do
    page=$(basename "$path" .html)
    [[ $page == *.es || $page == *.en ]] || continue
    if ! cmp -s "$path" "$before/$page.html"; then
        changed+=("$page")
    fi
done
removed=()
for path in "$before"/*.html; do
    page=$(basename "$path" .html)
    [[ $page == *.es || $page == *.en ]] || continue
    [[ -e "$after/$page.html" ]] || removed+=("$page")
done

# Only the first few get screenshots, so plain states in Spanish, the page's
# own language, come before their variants and before English.
ordered=()
while read -r page; do
    [[ -n $page ]] && ordered+=("$page")
done < <(for page in "${changed[@]}"; do
    variants=${page//[^.]/}
    printf '%s %s %s\n' "$([[ $page == *.en ]] && echo 1 || echo 0)" "${#variants}" "$page"
done | sort | cut -d' ' -f3)
changed=("${ordered[@]}")

screenshot() {
    local page=$1 side=$2 dir=$3
    npx --yes "$playwright" screenshot --wait-for-timeout 1000 --viewport-size "390, 844" --full-page \
        --color-scheme light "file://$dir/$page.html" "$out/$page.$side.phone-light.png" >/dev/null
    npx --yes "$playwright" screenshot --wait-for-timeout 1000 --viewport-size "390, 844" --full-page \
        --color-scheme dark "file://$dir/$page.html" "$out/$page.$side.phone-dark.png" >/dev/null
    npx --yes "$playwright" screenshot --wait-for-timeout 1000 --viewport-size "1280, 800" \
        --color-scheme light "file://$dir/$page.html" "$out/$page.$side.desktop.png" >/dev/null
}

cell() {
    local page=$1 side=$2 shot=$3 width=$4
    if [[ -e "$out/$page.$side.$shot.png" ]]; then
        printf '<img src="%s/%s.%s.%s.png" width="%s">' "$image_url" "$page" "$side" "$shot" "$width"
    else
        printf 'New state'
    fi
}

{
    echo '<!-- page-preview -->'
    echo '### Page preview'
    echo
    if [[ ${#changed[@]} -eq 0 && ${#removed[@]} -eq 0 ]]; then
        echo "Every page renders as it does on \`$base_name\`."
    else
        echo "Pages that render differently from \`$base_name\`, by state and language."
    fi
    open=" open"
    for page in "${changed[@]:0:$max_pages}"; do
        [[ -e "$before/$page.html" ]] && screenshot "$page" before "$before"
        screenshot "$page" after "$after"
        echo
        echo "<details$open><summary><code>$page</code></summary>"
        echo
        echo '| | Before | After |'
        echo '|---|---|---|'
        echo "| Phone, light | $(cell "$page" before phone-light 260) | $(cell "$page" after phone-light 260) |"
        echo "| Phone, dark | $(cell "$page" before phone-dark 260) | $(cell "$page" after phone-dark 260) |"
        echo "| Desktop | $(cell "$page" before desktop 400) | $(cell "$page" after desktop 400) |"
        echo
        echo '</details>'
        open=""
    done
    if [[ ${#changed[@]} -gt $max_pages ]]; then
        echo
        echo "Also changed: $(printf '`%s`, ' "${changed[@]:$max_pages}" | sed 's/, $//')."
    fi
    if [[ ${#removed[@]} -gt 0 ]]; then
        echo
        echo "No longer rendered: $(printf '`%s`, ' "${removed[@]}" | sed 's/, $//')."
    fi
} >"$out/comment.md"

echo $((${#changed[@]} + ${#removed[@]}))
