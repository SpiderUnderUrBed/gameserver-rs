#!/bin/bash
set -e

# 2. Create a default cluster if it doesn't exist
if ! k3d cluster get dev-cluster &> /dev/null; then
    k3d cluster create dev-cluster \
        --api-port 6443 \
        --port 8080:80@loadbalancer \
        --wait
fi

k3d completion bash > ~/completion-for-k3d.bash
echo 'source ~/completion-for-k3d.bash' >> ~/.bashrc

echo "Development environment is ready!"