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

DELETE FROM "plugin_cells";
DELETE FROM "plugins";
DELETE FROM "cells";

ALTER TABLE "cells" ADD COLUMN "world_id" INTEGER REFERENCES "worlds"(id);
ALTER TABLE "cells" ADD COLUMN "master" VARCHAR(255) NOT NULL;
CREATE UNIQUE INDEX "cells_unique_form_id_master_and_world_id" ON "cells" ("form_id", "master", "world_id");
DROP INDEX "cells_unique_form_id";

ALTER TABLE "plugins" ADD COLUMN "file_name" VARCHAR(255) NOT NULL;
ALTER TABLE "plugins" ADD COLUMN "file_path" TEXT NOT NULL;
ALTER TABLE "plugins" ALTER COLUMN "version" SET NOT NULL;
ALTER TABLE "plugins" ALTER COLUMN "masters" SET NOT NULL;
DROP INDEX "plugins_unique_name_and_file_id";
CREATE UNIQUE INDEX "plugins_unique_file_id_and_file_path" ON "plugins" ("file_id", "file_path");