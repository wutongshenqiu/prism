# Deploy To The SSH Host

Prism production deployment is currently a single-host Docker Compose deployment over SSH.

It is **not** an AWS ECS deployment.

## Source Of Truth

- GitHub workflow: [`.github/workflows/cd.yml`](/Users/qiufeng/work/proxy/prism/.github/workflows/cd.yml)
- Remote host alias: `aly-qf-ecs`
- Remote app root: `/opt/prism`

## Runtime Layout On The Host

- Live runtime config: `/opt/prism/data/config.yaml`
- Static frontend files: `/opt/prism/web-dist`
- Compose file: `/opt/prism/docker-compose.yml`
- Caddy config: `/opt/bazaar/Caddyfile`

The file `/opt/prism/config.yaml` is not part of the live runtime path and should not be used as the deployed config source of truth.

## GitHub Secrets

The deployment workflow uses SSH-specific repository secrets:

- `DEPLOY_SSH_HOST`
- `DEPLOY_SSH_USER`
- `DEPLOY_SSH_KEY`

If older `ECS_*` secrets still exist in the repository, treat them as obsolete compatibility leftovers rather than the source of truth.

## What The CD Workflow Does

On every push to `main`, the CD workflow:

1. builds and pushes `ghcr.io/wutongshenqiu/prism:latest`
2. SSHes into the remote host
3. updates `/opt/prism/web-dist` from the image contents
4. runs `docker compose down && docker compose up -d`
5. waits for the `prism-prism-1` Docker health check to become `healthy`

## Verification Commands

### Check Workflow Status

```bash
gh run list --workflow cd.yml --limit 5
gh run view <run-id>
```

### Check The Remote Host

```bash
ssh aly-qf-ecs 'cd /opt/prism && docker compose ps'
ssh aly-qf-ecs 'docker inspect prism-prism-1 --format "{{.State.Status}} {{.State.Health.Status}}"'
ssh aly-qf-ecs 'ls -lah /opt/prism/web-dist'
```

### Check The Live Site Through Caddy

```bash
ssh aly-qf-ecs 'curl -sk --resolve prism.qiufeng.cc:443:127.0.0.1 https://prism.qiufeng.cc/health'
ssh aly-qf-ecs 'curl -sk --resolve prism.qiufeng.cc:443:127.0.0.1 https://prism.qiufeng.cc/'
```

## Dashboard Credentials

Dashboard credentials are read from the live runtime config:

- username path: `dashboard.username`
- password hash path: `dashboard.password-hash`

If you need to rotate the dashboard password, update `/opt/prism/data/config.yaml` and restart the compose service.
