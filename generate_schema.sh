#!/bin/bash
# Generates the Diesel src/schema.rs file from a template SQL schema.
#
# It works by:
# 1. Creating a temporary database
# 2. Applying the schema template (replacing ___SCHEMA___ with 'public')
# 3. Running diesel print-schema against this temporary database
# 4. Cleaning up all temporary resources
#
# Requirements:
# - .env file with POSTGRES_URI defined
# - psql command-line tool
# - diesel CLI
# - rustfmt
#
# Usage: ./generate_schema.sh

set -euxo pipefail

# Source environment variables
source .env

SCHEMA_SOURCE="spacetraders_schema.sql.template"
SCHEMA_TARGET="src/schema.rs"
TEMP_DATABASE="spacetraders_tmp_$(date +%s)"
TEMP_CONNECTION_URI="${POSTGRES_URI%/*}/$TEMP_DATABASE"

# Create a temporary database
echo "Creating temporary database $TEMP_DATABASE"
psql "$POSTGRES_URI" -c "CREATE DATABASE $TEMP_DATABASE;"

# Apply schema to temporary database
echo "Applying schema to temporary database"
echo "Creating temporary connection string $TEMP_CONNECTION_URI"
sed 's/___SCHEMA___/public/g' "$SCHEMA_SOURCE" > temp_schema.sql
psql "$TEMP_CONNECTION_URI" -f temp_schema.sql

# Run diesel print-schema
echo "Generating schema file"
export DATABASE_URL="$TEMP_CONNECTION_URI"
diesel print-schema > "$SCHEMA_TARGET"
touch "$SCHEMA_TARGET"
rustfmt "$SCHEMA_TARGET"

# Clean up
echo "Dropping temporary database $TEMP_DATABASE"
psql "$POSTGRES_URI" -c "DROP DATABASE $TEMP_DATABASE;"
rm temp_schema.sql

echo "Schema generation complete!"
