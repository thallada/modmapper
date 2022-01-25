ALTER TABLE "plugins" ALTER COLUMN "mod_id" SET NOT NULL;
ALTER TABLE "plugins" ADD CONSTRAINT "plugins_mod_id_fkey" FOREIGN KEY ("mod_id") REFERENCES "mods" ("id");
ALTER TABLE "plugin_cells" ALTER COLUMN "file_id" SET NOT NULL;
ALTER TABLE "plugin_cells" ADD CONSTRAINT "plugin_cells_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id");
ALTER TABLE "plugin_cells" ALTER COLUMN "mod_id" SET NOT NULL;
ALTER TABLE "plugin_cells" ADD CONSTRAINT "plugin_cells_mod_id_fkey" FOREIGN KEY ("mod_id") REFERENCES "mods" ("id");