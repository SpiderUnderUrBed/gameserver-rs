#POD_NAME="rust-app-6b6c6bb656-44gf9" && \
tar --exclude='./target' --exclude='.git' --exclude='*.gitignore' -czf app.tar.gz . && \
kubectl cp app.tar.gz default/$POD_NAME:/usr/src/app && \
kubectl exec -n default -it $POD_NAME -- tar -xzvf /usr/src/app/app.tar.gz -C /usr/src/app && \
kubectl exec -n default -it $POD_NAME -- rm /usr/src/app/app.tar.gz
