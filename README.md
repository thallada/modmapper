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

7. Either run `cargo run` for development mode, or build the release binary with `cargo build --release`, which will get saved to `target/release/`.
