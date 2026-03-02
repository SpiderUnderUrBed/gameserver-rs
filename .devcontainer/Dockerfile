FROM mcr.microsoft.com/devcontainers/rust:2-1-bookworm

# Install k3d directly into the image
RUN curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | TAG=v5.8.3 bash

# Force the alternatives system to use NFT and set it to 'auto' mode
RUN apt-get update && apt-get install -y iptables \
    && update-alternatives --set iptables /usr/sbin/iptables-nft \
    && update-alternatives --auto iptables

# Setup k3d directory for the vscode user
RUN mkdir -p /home/vscode/.k3d && chown vscode /home/vscode/.k3d

# Ensure the Docker volume is defined for dind
VOLUME ["/var/lib/docker"]

# Setting the ENTRYPOINT to docker-init.sh will start up the Docker Engine 
ENTRYPOINT [ "/usr/local/share/docker-init.sh" ]