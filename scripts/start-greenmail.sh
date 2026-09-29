#!/usr/bin/env bash
# Start a local GreenMail IMAP/SMTP server for mymail development.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

GREENMAIL_VERSION="${GREENMAIL_VERSION:-2.1.3}"
JAR_DIR="${ROOT}/.tools"
JAR="${JAR_DIR}/greenmail-standalone-${GREENMAIL_VERSION}.jar"
USERS_OPT="-Dgreenmail.users=testuser:testpass@localhost"

if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
  echo "Starting GreenMail with docker compose..."
  docker compose up -d
  echo "Waiting for IMAP on 127.0.0.1:3143..."
  for _ in $(seq 1 40); do
    if (echo > /dev/tcp/127.0.0.1/3143) >/dev/null 2>&1; then
      echo "GreenMail is ready."
      echo "  IMAP  127.0.0.1:3143  user=testuser  pass=testpass"
      echo "  SMTP  127.0.0.1:3025"
      exit 0
    fi
    sleep 0.5
  done
  echo "GreenMail did not become ready in time." >&2
  docker compose logs
  exit 1
fi

if ! command -v java >/dev/null 2>&1; then
  echo "Neither Docker nor Java is available. Install Docker (recommended) or a JRE 17+." >&2
  exit 1
fi

mkdir -p "$JAR_DIR"
if [[ ! -f "$JAR" ]]; then
  url="https://repo1.maven.org/maven2/com/icegreen/greenmail-standalone/${GREENMAIL_VERSION}/greenmail-standalone-${GREENMAIL_VERSION}.jar"
  echo "Downloading GreenMail standalone from Maven Central..."
  curl -fsSL -o "$JAR" "$url"
fi

echo "Starting GreenMail with Java (no Docker)..."
exec java \
  -Dgreenmail.setup.test.all \
  -Dgreenmail.hostname=0.0.0.0 \
  ${USERS_OPT} \
  -jar "$JAR"
