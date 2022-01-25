/* TODO: make these non-nullable and add foreign keys */
ALTER TABLE "plugins" ADD COLUMN "mod_id" INTEGER;
ALTER TABLE "plugin_cells" ADD COLUMN "file_id" INTEGER;
ALTER TABLE "plugin_cells" ADD COLUMN "mod_id" INTEGER;