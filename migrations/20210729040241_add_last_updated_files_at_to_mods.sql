ALTER TABLE "mods" ADD COLUMN "last_updated_files_at" TIMESTAMP(3);

/* Backfill existing columns using the updated_at timestamps.
 *
 * This is approximate since usually the files are downloaded shortly after the mod record was created.
 * I mostly only care whether it is null or not null. Most existing mods need non-null values.
 */
UPDATE "mods" SET "last_updated_files_at" = "updated_at";
