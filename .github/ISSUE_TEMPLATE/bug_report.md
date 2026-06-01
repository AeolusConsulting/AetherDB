---
name: Bug Report
about: Report a bug in AetherDB
title: "[Bug] "
labels: bug
---

**Describe the bug**
A clear description of what the bug is.

**To reproduce**
```bash
curl -X POST https://your-worker.workers.dev/v1/... \
  -H "Authorization: Bearer TOKEN" \
  -H "Content-Type: application/json" \
  -d '{...}'
```

**Expected behavior**
What you expected to happen.

**Actual behavior**
What actually happened. Include the response body and status code.

**Environment**
- Deployment: CF Worker / Docker / Other
- Wrangler version: 
- SDK: TypeScript / Python / curl / Other
