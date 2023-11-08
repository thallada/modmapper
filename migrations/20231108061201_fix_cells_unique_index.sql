DROP INDEX "cells_unique_form_id_master_and_world_id";
CREATE UNIQUE INDEX "cells_unique_form_id_master_and_world_id" ON "cells" ("form_id", "master", "world_id") NULLS NOT DISTINCT;
