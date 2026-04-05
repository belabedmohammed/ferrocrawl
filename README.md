# Ferrocrawl

High-performance web scraping and data extraction REST API built in Rust. Deployed as AWS Lambda (ARM64) via Serverless Framework — scales from 0 to millions, pay-per-request.

## Architecture

```
                    API Gateway (HTTP API)
                           │
                           ▼
              ┌─────────────────────────┐
              │   AWS Lambda (ARM64)    │
              │   provided.al2023       │
              │                         │
              │  ┌───────────────────┐  │
              │  │   Axum Router     │  │
              │  │  /health          │  │
              │  │  /v1/scrape       │  │
              │  │  /v1/extract      │  │
              │  └────────┬──────────┘  │
              │           │             │
              │  ┌────────▼──────────┐  │
              │  │  StaticScraper    │  │
              │  │  reqwest + moka   │  │
              │  └────────┬──────────┘  │
              │           │             │
              │  ┌────────▼──────────┐  │
              │  │ ContentCleaner    │  │
              │  │ HTML → Markdown   │  │
              │  └────────┬──────────┘  │
              │           │             │
              │  ┌────────▼──────────┐  │
              │  │  LlmExtractor     │  │
              │  │  Claude API       │  │
              │  └───────────────────┘  │
              └─────────────────────────┘
```

## API Endpoints

| Method | Path          | Description                              |
|--------|---------------|------------------------------------------|
| GET    | `/health`     | Health check                             |
| POST   | `/v1/scrape`  | Scrape URL → clean markdown + metadata   |
| POST   | `/v1/extract` | Scrape URL → structured JSON via LLM     |

## Prerequisites

- Rust 1.84+
- cargo-lambda (`pip3 install cargo-lambda`)
- Serverless Framework 4 (`npm i -g serverless@4`)
- AWS CLI v2
- Docker (for local dev)

## Quick Start

### Local development

```bash
cp .env.example .env
# Edit .env with your keys

# Run locally (HTTP server on port 3400)
cargo run --bin local-server

# Or via Docker
docker compose up
```

### Deploy to AWS

```bash
# Build Lambda binary (ARM64)
cargo lambda build --release --arm64 --bin ferrocrawl

# Deploy
serverless deploy --stage prod
```

## API Usage

### Scrape a URL

```bash
curl -X POST https://your-api.execute-api.us-east-1.amazonaws.com/v1/scrape \
  -H "Content-Type: application/json" \
  -d '{"url": "https://example.com"}'
```

### Extract structured data

```bash
curl -X POST https://your-api.execute-api.us-east-1.amazonaws.com/v1/extract \
  -H "Content-Type: application/json" \
  -d '{
    "url": "https://news.ycombinator.com",
    "prompt": "Extract top 5 stories with title and points",
    "schema": {
      "type": "object",
      "properties": {
        "stories": {
          "type": "array",
          "items": {
            "type": "object",
            "properties": {
              "title": {"type": "string"},
              "points": {"type": "integer"}
            }
          }
        }
      }
    }
  }'
```

## CI/CD

| Workflow | Trigger | What it does |
|----------|---------|--------------|
| `ci.yml` | PR + push to main | fmt check, clippy, tests |
| `deploy.yml` | Push to main / manual | test → cargo-lambda build → serverless deploy → smoke test |

### Deployment stages

Manual deploy via `workflow_dispatch` supports `dev`, `staging`, `prod`.

### Required GitHub Secrets

| Secret | Description |
|--------|-------------|
| `AWS_ROLE_ARN` | IAM role ARN for OIDC (GitHub Actions) |
| `ANTHROPIC_API_KEY` | Claude API key (synced to SSM) |

## Environment Variables

| Variable                  | Default          | Description                        |
|---------------------------|------------------|------------------------------------|
| `FERROCRAWL_HOST`         | `0.0.0.0`       | Server bind host (local only)      |
| `FERROCRAWL_PORT`         | `3400`           | Server bind port (local only)      |
| `FERROCRAWL_TIMEOUT_SECS` | `30`            | HTTP request timeout               |
| `FERROCRAWL_MAX_BODY_SIZE`| `10485760`       | Max response body (10MB)           |
| `FERROCRAWL_CACHE_TTL`    | `300`            | Cache TTL in seconds               |
| `FERROCRAWL_CACHE_MAX`    | `1000`           | Max cached entries                 |
| `FERROCRAWL_API_KEYS`     | _(empty)_        | Comma-separated API keys for auth  |
| `ANTHROPIC_API_KEY`       | _(empty)_        | Required for `/v1/extract`         |
| `ANTHROPIC_MODEL`         | `claude-sonnet-4-20250514` | Claude model for extraction |

## Testing

```bash
cargo test
```

57 unit tests covering:
- SSRF protection (localhost, private IPs, AWS metadata endpoint)
- URL validation (scheme, format)
- HTML cleaning (scripts, ads, nav, cookies, modals)
- Metadata extraction (title, description, OG tags, canonical)
- LLM JSON block parsing
- Config redaction (API keys hidden in debug output)
- Error status code mapping

## Project Structure

```
ferrocrawl/
├── src/
│   ├── lib.rs                  # Shared router, state, middleware
│   ├── main.rs                 # Lambda entrypoint (production)
│   ├── local.rs                # HTTP server entrypoint (development)
│   ├── config.rs               # Nested config with redacted Debug
│   ├── error.rs                # Error types with logging
│   ├── routes/
│   │   ├── health.rs           # GET /health
│   │   ├── scrape.rs           # POST /v1/scrape
│   │   └── extract.rs          # POST /v1/extract
│   ├── scraper/
│   │   ├── static_scraper.rs   # reqwest + moka cache + SSRF protection
│   │   └── content_cleaner.rs  # HTML → clean markdown
│   └── extractor/
│       └── llm_extractor.rs    # Claude API structured extraction
├── serverless.yml              # AWS Lambda + API Gateway (ARM64)
├── .github/workflows/
│   ├── ci.yml                  # Lint + test on PR
│   └── deploy.yml              # Build + deploy + smoke test
├── Dockerfile                  # Local dev (local-server binary)
├── docker-compose.yml          # Local dev with env vars
├── Cargo.toml
└── .env.example
```
