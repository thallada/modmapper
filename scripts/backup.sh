#!/bin/bash
export $(grep -v '^#' .env | xargs -d '\n')
mkdir -p backups
zip -r -9 backups/plugins.zip plugins -DF --out backups/plugins-update.zip
pg_dump -h localhost -U modmapper -Fc modmapper > backups/modmapper-$(date +'%Y-%m-%d').dump
find backups/modmapper-*.dump -mtime +30 -type f -delete
rclone sync backups/* dropbox:modmapper