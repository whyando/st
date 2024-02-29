#!/bin/bash
set -euxo pipefail

# deploy with rsync and systemd over ssh

source .env set

if [ -z "$SSH_DEPLOY_TARGET" ]; then
    echo "SSH_DEPLOY_TARGET is not set"
    exit 1
fi

echo "Building release binary"
cargo build --release

ssh $SSH_DEPLOY_TARGET -- "mkdir -p /opt/st"

echo "Deploying to $SSH_DEPLOY_TARGET"
rsync -avzP target/release/main $SSH_DEPLOY_TARGET:/opt/st
rsync -avzP .env.remote $SSH_DEPLOY_TARGET:/opt/st/.env
rsync -avzP deploy/st.service $SSH_DEPLOY_TARGET:/etc/systemd/system/st.service

echo "Restarting service"
ssh $SSH_DEPLOY_TARGET -- "systemctl daemon-reload && systemctl restart st"
echo "Done"
