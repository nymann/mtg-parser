#!/usr/bin/env sh
set -eu

: "${TELEGRAM_BOT_TOKEN:?set TELEGRAM_BOT_TOKEN in .telegram-env}"
: "${TELEGRAM_CHAT_ID:?set TELEGRAM_CHAT_ID in .telegram-env}"

title="${PHASE_NOTIFY_TITLE:-mtg-parser}"
body="${PHASE_NOTIFY_BODY:-}"

if [ -n "$body" ]; then
  text="${title}

${body}"
else
  text="$title"
fi

curl_config="$(mktemp)"
trap 'rm -f "$curl_config"' EXIT
{
  printf 'url = "https://api.telegram.org/bot%s/sendMessage"\n' "$TELEGRAM_BOT_TOKEN"
  printf 'request = "POST"\n'
} >"$curl_config"

curl --fail --silent --show-error \
  --config "$curl_config" \
  --data-urlencode "chat_id=${TELEGRAM_CHAT_ID}" \
  --data-urlencode "text=${text}" \
  --data "disable_web_page_preview=true" \
  >/dev/null
