#!/bin/bash
set -e

# Get the current container ID
CONTAINER_ID=$(hostname)

# Create a default cluster if it doesn't exist
if ! k3d cluster get dev-cluster &> /dev/null; then
    k3d cluster create dev-cluster \
        --api-port 6443 \
        --port 8080:80@loadbalancer \
        --wait
fi

# Join the app container to the k3d network
# This allows 'app' to resolve 'k3d-dev-cluster-server-0'
docker network connect k3d-dev-cluster $CONTAINER_ID || true

mkdir -p ~/.kube
# Point kubectl to the cluster
# Because of 'network_mode: service:k3d', 0.0.0.0:6443 will actually work!
k3d kubeconfig get dev-cluster > ~/.kube/config

# Replace the host/port with the internal container name
# We target the server-0 container directly on the internal port 6443
sed -i "s|server: https://0.0.0.0:.*|server: https://k3d-dev-cluster-server-0:6443|g" ~/.kube/config

# Trust the internal cert (which is usually issued for 127.0.0.1)
kubectl config set-cluster k3d-dev-cluster --insecure-skip-tls-verify=true

# Add completions
k3d completion bash > ~/completion-for-k3d.bash
echo 'source ~/completion-for-k3d.bash' >> ~/.bashrc

echo "Development environment is ready!"