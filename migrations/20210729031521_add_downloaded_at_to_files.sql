ALTER TABLE "files" ADD COLUMN "downloaded_at" TIMESTAMP(3);

/* Backfill existing columns the created_at timestamps.
 *
 * This is approximate since usually the file was downloaded shortly after the record was created.
 * I mostly only care whether it is null or not null. All existing files need non-null values.
 */
UPDATE "files" SET "downloaded_at" = "created_at";