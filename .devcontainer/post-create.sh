#!/bin/bash
set -e

# Enable corepack
corepack enable
corepack install

# Create a default cluster if it doesn't exist
if ! k3d cluster get dev-cluster &> /dev/null; then
    k3d cluster create --config scripts/k3d-config.yaml || true
fi


mkdir -p ~/.kube
k3d kubeconfig get dev-cluster > ~/.kube/config
chmod 600 ~/.kube/config

echo 'export CARGO_HOME="$HOME/.cargo"' >> ~/.bashrc

# Add completions
mkdir -p ~/.local/share/bash-completion/completions/
printf '. <(k3d completion bash)\n'       >~/.local/share/bash-completion/completions/k3d
printf '. <(rustup completions bash)\n'       >~/.local/share/bash-completion/completions/rustup
printf '. <(rustup completions bash cargo)\n' >~/.local/share/bash-completion/completions/cargo

echo "Development environment is ready!"