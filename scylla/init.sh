#!/bin/bash

# Initialize Scylla schema
echo "Initializing Scylla schema..."

# Get Scylla connection details from environment or use defaults
SCYLLA_HOST=${SCYLLA_HOST:-"127.0.0.1"}
SCYLLA_PORT=${SCYLLA_PORT:-"9042"}

# Create the schema
cqlsh "$SCYLLA_HOST" "$SCYLLA_PORT" -f scylla/schema.cql

echo "Scylla schema initialized successfully!" 
