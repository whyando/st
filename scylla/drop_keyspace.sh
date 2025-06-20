#!/bin/bash

# Get Scylla connection details from environment or use defaults
SCYLLA_HOST=${SCYLLA_HOST:-"127.0.0.1"}
SCYLLA_PORT=${SCYLLA_PORT:-"9042"}

echo "⚠️  WARNING: This will DELETE ALL DATA in the spacetraders keyspace!"
echo "   Host: $SCYLLA_HOST:$SCYLLA_PORT"
echo "   Keyspace: spacetraders"
echo ""
echo "This will drop:"
echo "  - All tables (event_logs, current_state, events, snapshots)"
echo "  - Materialized view (events_by_entity_id)"
echo "  - Secondary index (events_timestamp_idx)"
echo "  - The entire spacetraders keyspace"
echo ""
read -p "Are you sure you want to reset the schema on $SCYLLA_HOST:$SCYLLA_PORT? (yes/no): " confirm

if [[ $confirm != "yes" ]]; then
    echo "Schema reset cancelled."
    exit 0
fi
echo "Dropping Scylla schema..."

# Drop everything
cqlsh "$SCYLLA_HOST" "$SCYLLA_PORT" -e "DROP KEYSPACE IF EXISTS spacetraders;"

echo "Scylla keyspace dropped successfully!"
