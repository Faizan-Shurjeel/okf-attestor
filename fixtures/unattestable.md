---
type: Attested Computation
title: Unsupported canonical runtime fixture
runtime: bigquery
parameters: []
executor:
  resource: skills/run-on-bq.md
  receipt: [job_id, executed_sql, result]
attester:
  resource: attesters/sql_equality.py
---

# Computation

```sql
SELECT 42
```
