CREATE TABLE IF NOT EXISTS "worlds" (
    "id" SERIAL PRIMARY KEY NOT NULL,
    "form_id" INTEGER NOT NULL,
    "master" VARCHAR(255) NOT NULL,
    "created_at" timestamp(3) NOT NULL,
    "updated_at" timestamp(3) NOT NULL
);
CREATE UNIQUE INDEX "worlds_unique_form_id_and_master" ON "worlds" ("form_id", "master");

CREATE TABLE IF NOT EXISTS "plugin_worlds" (
    "id" SERIAL PRIMARY KEY NOT NULL,
    "plugin_id" INTEGER REFERENCES "plugins"(id) NOT NULL,
    "world_id" INTEGER REFERENCES "worlds"(id) NOT NULL,
    "editor_id" VARCHAR(255) NOT NULL,
    "created_at" timestamp(3) NOT NULL,
    "updated_at" timestamp(3) NOT NULL
);
CREATE UNIQUE INDEX "plugin_worlds_unique_plugin_id_and_world_id" ON "plugin_worlds" ("plugin_id", "world_id");
CREATE INDEX "plugin_worlds_world_id" ON "plugin_worlds" ("world_id");

ALTER TABLE "cells" ADD COLUMN "world_id" INTEGER REFERENCES "worlds"(id);
CREATE UNIQUE INDEX "cells_unique_form_id_and_world_id" ON "cells" ("form_id", "world_id");
DROP INDEX "cells_unique_form_id";