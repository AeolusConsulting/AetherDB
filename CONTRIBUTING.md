# Contributing to AetherDB

Thanks for your interest in contributing! Here's how to get started.

## Development Setup

### CF Worker (edge deployment)

This is the primary deployment target.

```bash
# 1. Clone the repo
git clone https://github.com/AeolusConsulting/AetherDB.git
cd AetherDB/cf-worker

# 2. Install dependencies
npm install

# 3. Copy and configure wrangler
cp wrangler.toml.example wrangler.toml
# Edit wrangler.toml with your Cloudflare account ID and D1 database ID

# 4. Create the D1 database
npx wrangler d1 create aetherdb

# 5. Create the Vectorize index
npx wrangler vectorize create aetherdb-vectors --dimensions=768 --metric=cosine

# 6. Create Vectorize metadata indexes
npx wrangler vectorize create-metadata-index aetherdb-vectors --property-name=source --type=string
npx wrangler vectorize create-metadata-index aetherdb-vectors --property-name=category --type=string
npx wrangler vectorize create-metadata-index aetherdb-vectors --property-name=created_at --type=number

# 7. Run the D1 migrations
npx wrangler d1 execute aetherdb --file=../migrations/001_documents.sql
npx wrangler d1 execute aetherdb --file=../migrations/003_idempotency.sql
npx wrangler d1 execute aetherdb --file=../migrations/004_entities.sql
npx wrangler d1 execute aetherdb --file=../migrations/005_relationships.sql

# 8. Set Worker secrets
npx wrangler secret put API_TOKEN
npx wrangler secret put ADMIN_TOKEN

# 9. Deploy
npx wrangler deploy

# 10. Initialize FTS and versioning tables
curl -X POST https://your-worker.workers.dev/v1/admin/init-schema \
  -H "Authorization: Bearer YOUR_ADMIN_TOKEN"
```

### Rust Stack (local development)

```bash
cp .env.example .env
docker compose up -d
```

### Running Locally (CF Worker dev mode)

```bash
cd cf-worker
npx wrangler dev --remote
```

## Making Changes

1. Fork the repo and create a feature branch
2. Make your changes in `cf-worker/src/index.js` (or relevant files)
3. Test locally with `npx wrangler dev --remote`
4. Ensure no secrets or personal URLs are in your changes
5. Submit a pull request

## Pull Request Guidelines

- Keep PRs focused on a single change
- Update documentation if you add/change endpoints
- Update the OpenAPI spec (`docs/openapi.yaml`) for API changes
- Update SDKs (`sdks/`) if the API contract changes
- Test the happy path and edge cases before submitting

## Project Structure

```
cf-worker/          Cloudflare Workers deployment (primary)
  src/index.js      All API logic in a single file
  wrangler.toml.example  Template configuration
apps/               Rust applications (api-gateway, workers, etc.)
crates/             Rust library crates
mcp-server/         MCP server for AI assistants (12 tools)
sdks/               TypeScript and Python client SDKs
docs/               OpenAPI spec, architecture, integration guide
migrations/         D1/libSQL schema migrations
```

## Reporting Issues

- Use the [bug report template](.github/ISSUE_TEMPLATE/bug_report.md) for bugs
- Use the [feature request template](.github/ISSUE_TEMPLATE/feature_request.md) for ideas
- Check existing issues before filing a new one

## License

By contributing, you agree that your contributions will be licensed under the Apache-2.0 License.
