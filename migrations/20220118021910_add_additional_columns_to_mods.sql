ALTER TABLE "mods" ADD COLUMN "last_update_at" TIMESTAMP(3); /* TODO: make NOT NULL after backfill */
ALTER TABLE "mods" ADD COLUMN "first_upload_at" TIMESTAMP(3); /* TODO: make NOT NULL after backfill */
ALTER TABLE "mods" ADD COLUMN "thumbnail_link" VARCHAR(255);
ALTER TABLE "mods" ADD COLUMN "author_id" INTEGER; /* TODO: make NOT NULL after backfill */
ALTER TABLE "mods" ADD COLUMN "category_id" INTEGER;
ALTER TABLE "mods" RENAME COLUMN "author" TO "author_name";
ALTER TABLE "mods" RENAME COLUMN "category" TO "category_name";