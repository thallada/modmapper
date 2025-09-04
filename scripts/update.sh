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
        curl -fsS -m 10 --retry 5 --data-raw "$data" "https://hc-ping.com/${HEALTHCHECK_PING_KEY}/modmapper-update${endpoint}?rid=${RID}" >/dev/null || true
    else
        curl -fsS -m 10 --retry 5 "https://hc-ping.com/${HEALTHCHECK_PING_KEY}/modmapper-update${endpoint}?rid=${RID}" >/dev/null || true
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

if [ -f cells/edits.json ]; then
    last_update_time=$(date -r cells/edits.json +'%Y-%m-%dT%H:%M:%S')
fi
mkdir -p logs
./target/release/mod-mapper -g skyrimspecialedition &>> logs/modmapper.log
./target/release/mod-mapper -g skyrim &>> logs/modmapper.log
mkdir -p cells
mkdir -p mods
mkdir -p files
mkdir -p plugins_data
if [ -n "$last_update_time" ]; then
    ./target/release/mod-mapper -e cells/edits.json &>> logs/modmapper.log
    ./target/release/mod-mapper -c cells &>> logs/modmapper.log
    ./target/release/mod-mapper -s mods/skyrimspecialedition/mod_search_index.json -g skyrimspecialedition &>> logs/modmapper.log
    ./target/release/mod-mapper -s mods/skyrim/mod_search_index.json -g skyrim &>> logs/modmapper.log
    ./target/release/mod-mapper -M mods/mod_cell_counts.json &>> logs/modmapper.log
    ./target/release/mod-mapper -G mods/games.json &>> logs/modmapper.log
    ./target/release/mod-mapper -m mods -u "$last_update_time" &>> logs/modmapper.log
    ./target/release/mod-mapper -P plugins_data -u "$last_update_time" &>> logs/modmapper.log
    ./target/release/mod-mapper -F files -u "$last_update_time" &>> logs/modmapper.log
else
    ./target/release/mod-mapper -e cells/edits.json &>> logs/modmapper.log
    ./target/release/mod-mapper -c cells &>> logs/modmapper.log
    ./target/release/mod-mapper -s mods/skyrimspecialedition/mod_search_index.json -g skyrimspecialedition &>> logs/modmapper.log
    ./target/release/mod-mapper -s mods/skyrim/mod_search_index.json -g skyrim &>> logs/modmapper.log
    ./target/release/mod-mapper -M mods/mod_cell_counts.json &>> logs/modmapper.log
    ./target/release/mod-mapper -G mods/games.json &>> logs/modmapper.log
    ./target/release/mod-mapper -m mods &>> logs/modmapper.log
    ./target/release/mod-mapper -P plugins_data &>> logs/modmapper.log
    ./target/release/mod-mapper -F files &>> logs/modmapper.log
fi

# Send success ping
ping_healthcheck ""
