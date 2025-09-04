#!/bin/bash
set -e
set -o pipefail

# Load environment variables
export $(grep -v '^#' .env | xargs -d '\n')

# Generate UUID for this run
RID=$(cat /proc/sys/kernel/random/uuid)

# Healthchecks.io ping function
ping_healthcheck() {
    local endpoint="$1"
    local data="$2"
    if [ -n "$data" ]; then
        curl -fsS -m 10 --retry 5 --data-raw "$data" "https://hc-ping.com/${HEALTHCHECK_PING_KEY}/modmapper-sync${endpoint}?rid=${RID}" >/dev/null || true
    else
        curl -fsS -m 10 --retry 5 "https://hc-ping.com/${HEALTHCHECK_PING_KEY}/modmapper-sync${endpoint}?rid=${RID}" >/dev/null || true
    fi
}

# Send failure notification with logs
send_failure() {
    local logs=""
    if [ -f logs/modmapper.log ]; then
        logs=$(tail --bytes=100000 logs/modmapper.log)
    fi
    ping_healthcheck "/fail" "$logs"
    exit 1
}

# Trap to catch failures
trap send_failure ERR

# Send start ping
ping_healthcheck "/start"

rclone sync --fast-list --checksum cells ${STATIC_SERVER_REMOTE}:${STATIC_SERVER_CELLS_BUCKET}
rclone sync --fast-list --checksum mods ${STATIC_SERVER_REMOTE}:${STATIC_SERVER_MODS_BUCKET}
rclone sync --fast-list --checksum plugins_data ${STATIC_SERVER_REMOTE}:${STATIC_SERVER_PLUGINS_BUCKET}
rclone sync --fast-list --checksum files ${STATIC_SERVER_REMOTE}:${STATIC_SERVER_FILES_BUCKET}

# Send success ping
ping_healthcheck ""
