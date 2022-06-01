#!/bin/bash
mkdir -p logs
./target/release/mod-mapper &>> logs/modmapper.log
mkdir -p cells
mkdir -p mods
mkdir -p files
mkdir -p plugins_data
./target/release/mod-mapper -e cells/edits.json
./target/release/mod-mapper -c cells
./target/release/mod-mapper -s mods/mod_search_index.json
./target/release/mod-mapper -M mods/mod_cell_counts.json
./target/release/mod-mapper -m mods
./target/release/mod-mapper -P plugins_data
./target/release/mod-mapper -F files