#!/bin/bash
if [ -f cells/edits.json ]; then
    last_update_time=$(date -r cells/edits.json +'%Y-%m-%dT%H:%M:%S')
fi
mkdir -p logs
./target/release/mod-mapper &>> logs/modmapper.log
mkdir -p cells
mkdir -p mods
mkdir -p files
mkdir -p plugins_data
if [ -n "$last_update_time" ]; then
    ./target/release/mod-mapper -e cells/edits.json
    ./target/release/mod-mapper -c cells
    ./target/release/mod-mapper -s mods/mod_search_index.json
    ./target/release/mod-mapper -M mods/mod_cell_counts.json
    ./target/release/mod-mapper -m mods -u "$last_update_time"
    ./target/release/mod-mapper -P plugins_data -u "$last_update_time"
    ./target/release/mod-mapper -F files -u "$last_update_time"
else
    ./target/release/mod-mapper -e cells/edits.json
    ./target/release/mod-mapper -c cells
    ./target/release/mod-mapper -s mods/mod_search_index.json
    ./target/release/mod-mapper -M mods/mod_cell_counts.json
    ./target/release/mod-mapper -m mods
    ./target/release/mod-mapper -P plugins_data
    ./target/release/mod-mapper -F files
fi