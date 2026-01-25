# Schemas

## TaskRequest
Represents a human-originated request broken into requested changes.

```json
{
  "request_id": "REQ-1001",
  "summary": "Add API rate limit",
  "requested_changes": [
    {
      "path": "src/api/limits.rs",
      "summary": "Implement token bucket"
    }
  ]
}
```

## TaskAssignment
Represents a unit of work for a Developer agent.

```json
{
  "task_id": "REQ-1001-1",
  "parent_request_id": "REQ-1001",
  "summary": "Implement token bucket",
  "file_targets": ["src/api/limits.rs"],
  "instructions": [
    "Keep changes isolated to the listed files.",
    "Provide a structured JSON change request on completion."
  ]
}
```

## ChangeRequest
Represents a Developer-submitted change request.

```json
{
  "task_id": "REQ-1001-1",
  "agent": "developer-2",
  "changes": [
    {
      "path": "src/api/limits.rs",
      "operation": "update",
      "patch": "@@ -10,7 +10,8 @@\n- old\n+ new"
    }
  ],
  "checks": [
    "cargo test",
    "cargo clippy"
  ]
}
```

## ValidationResult
Describes validation outcomes for a change request.

```json
{
  "valid": true,
  "errors": []
}
```
