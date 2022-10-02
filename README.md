# modmapper

Downloads mods from nexus, parses the plugins inside, and saves data to a postgres database.

## Development Install

1. Install and run postgres.
2. Create postgres user and database (and add uuid extension while you're there
   ):

```
createuser modmapper
createdb modmapper
sudo -u postgres -i psql
postgres=# ALTER DATABASE modmapper OWNER TO modmapper;
\password modmapper

# Or, on Windows in PowerShell:

& 'C:\Program Files\PostgreSQL\13\bin\createuser.exe' -U postgres modmapper
& 'C:\Program Files\PostgreSQL\13\bin\createdb.exe' -U postgres modmapper
& 'C:\Program Files\PostgreSQL\13\bin\psql.exe' -U postgres
postgres=# ALTER DATABASE modmapper OWNER TO modmapper;
\password modmapper
```

3. Save password somewhere safe and then and add a `.env` file to the project
   directory with the contents:

```
DATABASE_URL=postgresql://modmapper:<password>@localhost/modmapper
RUST_LOG=mod_mapper=debug
```

4. Install
   [`sqlx_cli`](https://github.com/launchbadge/sqlx/tree/master/sqlx-cli) with
   `cargo install sqlx-cli --no-default-features --features postgres`
5. Run `sqlx migrate --source migrations run` which will run all the database migrations.
6. Get your personal Nexus API token from your profile settings and add it to the `.env` file:

```
NEXUS_API_KEY=...
```

7. Build the release binary by running `cargo build --release`.
8. See `./target/release/modmapper -h` for further commands or run `./scripts/update.sh` to start populating the database with scraped mods and dumping the data to JSON files.

## Nexus Mods user credentials

Nexus Mods filters out adult-only mods unless you are logged in and have set your content filters to allow adult mods. Modmapper works without Nexus Mods user credentials, but if you would like to add your user credentials so that adult mods are included then edit the `.env` file and add values for `NEXUS_MODS_USERNAME` and `NEXUS_MODS_PASSWORD`.

## Sync and Backup Setup

`scripts/sync.sh` and `scripts/backup.sh` both utilize [`rclone`](https://rclone.org) to transfer files that are generated on the machine running modmapper to separate servers for file storage.

For these scripts to run successfully you will need to install rclone and setup a remote for `sync.sh` (the "static server") and a remote for `backup.sh` (the "backup server"). Remotes can be created with the `rclone config` command. Then, make sure these variables are defined in the `.env` file corresponding to the remote names and buckets (or folders) within that remote you created:

- `STATIC_SERVER_REMOTE`
- `STATIC_SERVER_CELLS_BUCKET`
- `STATIC_SERVER_MODS_BUCKET`
- `STATIC_SERVER_PLUGINS_BUCKET`
- `STATIC_SERVER_FILES_BUCKET`
- `BACKUP_SERVER_REMOTE`
- `BACKUP_SERVER_BUCKET`
