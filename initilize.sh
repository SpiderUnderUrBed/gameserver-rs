#eval $(minikube docker-env)
#docker build . -t rust-k8s
# # Get minikube IP
# MINIKUBE_IP=$(kubectl get nodes -o wide --no-headers | awk '{print $6}')

# # Mount using SSHFS (run on your host machine)
# #sshfs -i $(minikube ssh-key) -o allow_other,default_permissions $(whoami)@$MINIKUBE_IP:/mnt/data/gameserver-rs ~/projects/gameserver-rs
# sshfs -o debug \
#       -o IdentityFile=$(minikube ssh-key) \
#       -o allow_other \
#       -o default_permissions \
#       -o StrictHostKeyChecking=no \
#       -o UserKnownHostsFile=/dev/null \
#       docker@$MINIKUBE_IP:/mnt/data/gameserver-rs \
#       ~/projects/gameserver-rs
#192.168.49.2

sshfs -o debug \
  -o IdentityFile=$(minikube ssh-key) \
  -o allow_other \
  -o default_permissions \
  -o StrictHostKeyChecking=no \
  -o UserKnownHostsFile=/dev/null \
  docker@$(minikube ip):/mnt/data/gameserver-rs \
  ~/projects/gameserver-rs
