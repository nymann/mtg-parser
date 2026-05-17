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

curl --fail --silent --show-error \
  --request POST "https://api.telegram.org/bot${TELEGRAM_BOT_TOKEN}/sendMessage" \
  --data-urlencode "chat_id=${TELEGRAM_CHAT_ID}" \
  --data-urlencode "text=${text}" \
  --data "disable_web_page_preview=true" \
  >/dev/null
