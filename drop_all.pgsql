/* !!! THIS DROPS ALL TABLES IN THE DATABASE WHICH DELETES ALL DATA IN THE DATABASE !!!
 *
 * ONLY RUN IN DEVELOPMENT!
 */
DROP TABLE _sqlx_migrations CASCADE;
DROP TABLE games CASCADE;
DROP TABLE mods CASCADE;
DROP TABLE files CASCADE;
DROP TABLE plugins CASCADE;
DROP TABLE cells CASCADE;
DROP TABLE worlds CASCADE;
DROP TABLE plugin_cells CASCADE;
DROP TABLE plugin_worlds CASCADE;