CREATE TABLE IF NOT EXISTS "games" (
    "id" SERIAL PRIMARY KEY NOT NULL,
    "name" VARCHAR(255) NOT NULL,
    "nexus_game_id" INTEGER NOT NULL,
    "created_at" timestamp(3) NOT NULL,
    "updated_at" timestamp(3) NOT NULL
);
CREATE UNIQUE INDEX "games_unique_name_and_nexus_game_id" ON "games" ("nexus_game_id", "name");

CREATE TABLE IF NOT EXISTS "mods" (
    "id" SERIAL PRIMARY KEY NOT NULL,
    "name" VARCHAR(255) NOT NULL,
    "author" VARCHAR(255) NOT NULL,
    "category" VARCHAR(255) NOT NULL,
    "description" TEXT,
    "nexus_mod_id" INTEGER NOT NULL,
    "game_id" INTEGER REFERENCES "games"(id) NOT NULL,
    "created_at" timestamp(3) NOT NULL,
    "updated_at" timestamp(3) NOT NULL
);
CREATE UNIQUE INDEX "mods_unique_game_id_and_nexus_mod_id" ON "mods" ("game_id", "nexus_mod_id");
CREATE INDEX "mods_nexus_mod_id" ON "mods" ("nexus_mod_id");

CREATE TABLE IF NOT EXISTS "files" (
    "id" SERIAL PRIMARY KEY NOT NULL,
    "name" VARCHAR(255) NOT NULL,
    "file_name" VARCHAR(255) NOT NULL,
    "nexus_file_id" INTEGER NOT NULL,
    "mod_id" INTEGER REFERENCES "mods"(id) NOT NULL,
    "category" VARCHAR(255),
    "version" VARCHAR(255),
    "mod_version" VARCHAR(255),
    "uploaded_at" timestamp(3) NOT NULL,
    "created_at" timestamp(3) NOT NULL,
    "updated_at" timestamp(3) NOT NULL
);
CREATE UNIQUE INDEX "files_unique_mod_id_and_nexus_file_id" ON "files" ("mod_id", "nexus_file_id");
CREATE INDEX "files_nexus_file_id" ON "files" ("nexus_file_id");

CREATE TABLE IF NOT EXISTS "plugins" (
    "id" SERIAL PRIMARY KEY NOT NULL,
    "name" VARCHAR(255) NOT NULL,
    "hash" BIGINT NOT NULL,
    "file_id" INTEGER REFERENCES "files"(id) NOT NULL,
    "version" FLOAT,
    "author" TEXT,
    "description" TEXT,
    "masters" VARCHAR(255)[],
    "created_at" timestamp(3) NOT NULL,
    "updated_at" timestamp(3) NOT NULL
);
CREATE UNIQUE INDEX "plugins_unique_name_and_file_id" ON "plugins" ("file_id", "name");
CREATE INDEX "plugins_name" ON "plugins" ("name");

CREATE TABLE IF NOT EXISTS "cells" (
    "id" SERIAL PRIMARY KEY NOT NULL,
    "form_id" INTEGER NOT NULL,
    "x" INTEGER,
    "y" INTEGER,
    "is_persistent" BOOLEAN NOT NULL,
    "created_at" timestamp(3) NOT NULL,
    "updated_at" timestamp(3) NOT NULL
);
CREATE UNIQUE INDEX "cells_unique_form_id" ON "cells" ("form_id");

CREATE TABLE IF NOT EXISTS "plugin_cells" (
    "id" SERIAL PRIMARY KEY NOT NULL,
    "plugin_id" INTEGER REFERENCES "plugins"(id) NOT NULL,
    "cell_id" INTEGER REFERENCES "cells"(id) NOT NULL,
    "editor_id" VARCHAR(255),
    "created_at" timestamp(3) NOT NULL,
    "updated_at" timestamp(3) NOT NULL
);
CREATE UNIQUE INDEX "plugin_cells_unique_plugin_id_and_cell_id" ON "plugin_cells" ("plugin_id", "cell_id");
CREATE INDEX "plugin_cells_cell_id" ON "plugin_cells" ("cell_id");