#!/bin/sh

ssh spiderunderurbed@192.168.68.38 << 'EOF'
set -e
cd /home/spiderunderurbed
if [ ! -d "gameserver-rs" ]; then
    git clone https://github.com/SpiderUnderUrBed/gameserver-rs.git
fi
cd gameserver-rs
if ! docker ps | grep -q "registry:2"; then
    docker run -d -p 5000:5000 --restart=always --name registry registry:2
fi
docker build -t rust-k8s:latest .
docker tag rust-k8s:latest localhost:5000/rust-k8s:latest
docker push localhost:5000/rust-k8s:latest || echo "Push failed, continuing with local image..."
kubectl apply -f .
EOF
