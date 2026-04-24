
# Contributors Guide: Project Workspace Setup

Welcome to the project! Follow these steps to set up your development environment:
## Ai policy

Any AI assistence of any form needs to be disclosed in any PR's. All contributers are fully accountable for the code they submit, whether its by an AI or not. Contributers are expected to have full understanding of the code they written and some of the relevent code surrounding their contribution.

## 1. Devcontainer Environment

We use [devcontainers](https://containers.dev/) to provide a consistent development environment.

Please ensure you have the [Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers) installed. When you open the project in VS Code, VSCode will ask you to open it using devcontainer, which will automatically set up the required tools and dependencies.

## 2. Kubernetes (k3d) Cluster Setup

A k3d Kubernetes cluster is provisioned automatically in the devcontainer. If you need to recreate the cluster manually, use the following commands:

```sh
k3d cluster delete dev-cluster
k3d cluster create --config scripts/k3d-config.yaml
```

This will delete any existing cluster named `dev-cluster` and create a new one using the configuration in `scripts/k3d-config.yaml`.

## 3. Deploying the Database

Once your k3d cluster is running, deploy the PostgreSQL 18 database using the provided kustomization file:

```sh
kubectl apply -k scripts/kustomization.yaml
```

This will set up the database in your cluster. You can verify the deployment with:

```sh
kubectl get pods -A
```

---

If you encounter any issues, please check the README or reach out to the maintainers for help.
