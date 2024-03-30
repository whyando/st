#!/bin/bash
set -euo pipefail

source .env set

# DIESEL
# autogenerates file 'src/schema.rs'
diesel print-schema > src/schema.rs
rustfmt src/schema.rs

# also use pg_dump to grab a backup copy of the schema
pg_dump "$DATABASE_URL" --schema-only --schema=public > spacetraders_schema.sql
    
