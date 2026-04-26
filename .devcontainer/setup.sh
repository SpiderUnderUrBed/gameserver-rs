#!/bin/bash
set -e
# cd "$(dirname "$0")/.."

export SECRET=test
export SITE_URL=/gameserver-rs
export TESTING=true
export POSTGRES_USER=gameserver
export POSTGRES_PASSWORD=gameserverpass
export POSTGRES_DB=gameserver_db
export POSTGRES_HOST=localhost
export POSTGRES_PORT=5432

kubectl apply -f scripts/postgres-dev.yaml
kubectl rollout status deployment/postgres-deployment --timeout=60s
docker build -f gameserver/Dockerfile.dev -t localhost:5000/gameserver:latest .
docker push localhost:5000/gameserver:latest
kubectl apply -f gameserver/deployment-dev.yaml
kubectl rollout status deployment/gameserver --timeout=120s
kubectl port-forward svc/gameserver-service 8080:8080 &
PF_PID=$!
sleep 5
cargo test --features full-stack
kill $PF_PID || true