#!/bin/bash
set -euxo pipefail

source .env set

echo "Stopping service"
ssh $SSH_DEPLOY_TARGET -- "systemctl restart st"
echo "Done"
