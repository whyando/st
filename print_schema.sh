#!/bin/bash
set -euo pipefail

source .env set

# autogenerates file 'src/schema.rs'
diesel print-schema > src/schema.rs
rustfmt src/schema.rs
