.PHONY: build dev test lint fmt clean check \
       docker-build docker-run docker-stop docker-logs \
       docker-compose-up docker-compose-down audit

build:
	cargo build --release

dev:
	cargo run -- --config config.yaml

test:
	cargo test --workspace

lint:
	cargo fmt --check
	cargo clippy --workspace -- -D warnings

fmt:
	cargo fmt

clean:
	cargo clean

check:
	cargo check --workspace

# Docker
docker-build:
	docker build -t ai-proxy:local .

docker-run:
	docker run -d --name ai-proxy -p 8317:8317 -v $(PWD)/config.yaml:/etc/ai-proxy/config.yaml:ro ai-proxy:local

docker-stop:
	docker stop ai-proxy && docker rm ai-proxy

docker-logs:
	docker logs -f ai-proxy

# Docker Compose
docker-compose-up:
	docker compose up -d --build

docker-compose-down:
	docker compose down

# Security
audit:
	cargo audit
