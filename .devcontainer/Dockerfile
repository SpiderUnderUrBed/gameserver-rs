FROM mcr.microsoft.com/devcontainers/rust:2-1-bookworm

# Install system dependencies first
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends gnupg2 wget ca-certificates \
    && install -d /usr/share/postgresql-common/pgdg \
    && wget --quiet -O - https://www.postgresql.org/media/keys/ACCC4CF8.asc | gpg --dearmor -o /usr/share/postgresql-common/pgdg/apt.postgresql.org.gpg \
    && echo "deb [signed-by=/usr/share/postgresql-common/pgdg/apt.postgresql.org.gpg] http://apt.postgresql.org/pub/repos/apt bookworm-pgdg main" > /etc/apt/sources.list.d/pgdg.list \
    && apt-get update \
    && apt-get -y install postgresql-client-18 \
    && apt-get clean -y && rm -rf /var/lib/apt/lists/*

# Add sqlx-cli
RUN cargo install sqlx-cli --no-default-features --features native-tls,postgres

# Install k3d directly into the image
RUN curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | TAG=v5.8.3 bash

# Setup k3d directory for the vscode user
RUN mkdir -p /home/vscode/.k3d && chown vscode /home/vscode/.k3d