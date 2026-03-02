#!/bin/bash
set -e

# Create a default cluster if it doesn't exist
if ! k3d cluster get dev-cluster &> /dev/null; then
    k3d cluster create --config scripts/k3d-config.yaml || true
fi


mkdir -p ~/.kube
k3d kubeconfig get dev-cluster > ~/.kube/config
chmod 600 ~/.kube/config

# Add completions
k3d completion bash > ~/completion-for-k3d.bash
echo 'source ~/completion-for-k3d.bash' >> ~/.bashrc

echo "Development environment is ready!"