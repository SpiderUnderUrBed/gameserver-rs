FROM mcr.microsoft.com/devcontainers/rust:2-1-bookworm

# Install k3d directly into the image
RUN curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | TAG=v5.8.3 bash

# Setup k3d directory for the vscode user
RUN mkdir -p /home/vscode/.k3d && chown vscode /home/vscode/.k3d