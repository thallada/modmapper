#!/bin/bash
if [ -f cells/edits.json ]; then
    last_update_time=$(date -r cells/edits.json +'%Y-%m-%dT%H:%M:%S')
fi
mkdir -p logs
./target/release/mod-mapper -g skyrimspecialedition &>> logs/modmapper.log
mkdir -p cells
mkdir -p mods
mkdir -p files
mkdir -p plugins_data
if [ -n "$last_update_time" ]; then
    ./target/release/mod-mapper -e cells/edits.json &>> logs/modmapper.log
    ./target/release/mod-mapper -c cells &>> logs/modmapper.log
    ./target/release/mod-mapper -s mods/mod_search_index.json &>> logs/modmapper.log
    ./target/release/mod-mapper -M mods/mod_cell_counts.json &>> logs/modmapper.log
    ./target/release/mod-mapper -G mods/games.json &>> logs/modmapper.log
    ./target/release/mod-mapper -m mods -u "$last_update_time" &>> logs/modmapper.log
    ./target/release/mod-mapper -P plugins_data -u "$last_update_time" &>> logs/modmapper.log
    ./target/release/mod-mapper -F files -u "$last_update_time" &>> logs/modmapper.log
else
    ./target/release/mod-mapper -e cells/edits.json &>> logs/modmapper.log
    ./target/release/mod-mapper -c cells &>> logs/modmapper.log
    ./target/release/mod-mapper -s mods/mod_search_index.json &>> logs/modmapper.log
    ./target/release/mod-mapper -M mods/mod_cell_counts.json &>> logs/modmapper.log
    ./target/release/mod-mapper -G mods/games.json &>> logs/modmapper.log
    ./target/release/mod-mapper -m mods &>> logs/modmapper.log
    ./target/release/mod-mapper -P plugins_data &>> logs/modmapper.log
    ./target/release/mod-mapper -F files &>> logs/modmapper.log
fi