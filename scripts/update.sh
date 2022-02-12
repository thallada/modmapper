#!/bin/bash
mkdir -p logs
./target/release/mod-mapper &>> logs/modmapper.log
mkdir -p cells
mkdir -p mods
./target/release/mod-mapper -e cells/edits.json
./target/release/mod-mapper -c cells
./target/release/mod-mapper -s mods/mod_search_index.json
./target/release/mod-mapper -m mods