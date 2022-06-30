#!/bin/bash
export $(grep -v '^#' .env | xargs -d '\n')
rclone sync --fast-list --checksum cells ${STATIC_SERVER_REMOTE}:${STATIC_SERVER_CELLS_BUCKET}
rclone sync --fast-list --checksum mods ${STATIC_SERVER_REMOTE}:${STATIC_SERVER_MODS_BUCKET}
rclone sync --fast-list --checksum plugins_data ${STATIC_SERVER_REMOTE}:${STATIC_SERVER_PLUGINS_BUCKET}
rclone sync --fast-list --checksum files ${STATIC_SERVER_REMOTE}:${STATIC_SERVER_FILES_BUCKET}