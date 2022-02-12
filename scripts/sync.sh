#!/bin/bash
export $(grep -v '^#' .env | xargs -d '\n')
rsync -raz -e "ssh -p ${STATIC_SERVER_PORT}" cells ${STATIC_SERVER_USER}@${STATIC_SERVER_HOST}:/srv/
rsync -raz -e "ssh -p ${STATIC_SERVER_PORT}" mods ${STATIC_SERVER_USER}@${STATIC_SERVER_HOST}:/srv/