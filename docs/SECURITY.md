# Rustclaw Security Model

## Layers

### 1. Gateway Authentication
- Bearer token authentication on all `/api/*` endpoints
- Timing-safe comparison to prevent timing attacks
- Configurable mode: `token` (production) or `none` (development only)

### 2. Channel Webhook Verification
- **Slack**: HMAC-SHA256 signature verification with replay protection (5-minute window)
- **WhatsApp**: HMAC-SHA256 signature verification (X-Hub-Signature-256)
- **Telegram**: Secret token header verification (X-Telegram-Bot-Api-Secret-Token)
- All verifications use constant-time comparison

### 3. Rate Limiting
- Per-IP sliding window (configurable RPM)
- Applied to all API endpoints
- Returns 429 when exceeded

### 4. Tool Execution Guard (guardd)
- **Policy Engine**: Rule-based authorization for all actions
- **Shell Allowlist**: Only pre-approved commands can execute (ls, cat, git, etc.)
- **Path Sandboxing**: File operations confined to workspace directory
- **Audit Logging**: Every guard decision logged to JSONL
- **Dangerous Command Detection**: Blocks rm -rf, sudo, chmod 777, etc.

### 5. Credential Storage
- AES-256-GCM encryption at rest
- Key rotation support without data loss
- Zeroize on drop for in-memory secrets

### 6. Input Validation
- Maximum message length: 32,000 characters
- JSON parsing with error handling (no panics)
- Path traversal prevention on file operations

## Threat Model

| Threat | Mitigation |
|--------|-----------|
| Unauthorized API access | Bearer token auth |
| Webhook spoofing | HMAC signature verification |
| Command injection | Shell allowlist + guardd policy |
| Path traversal | Workspace confinement |
| Replay attacks | Timestamp validation (5-min window) |
| Timing attacks | Constant-time comparison |
| Secret exposure | Encrypted credential store |
| DoS | Rate limiting per-IP |
| Audit evasion | Mandatory JSONL logging |

## Recommendations
1. Always use `gateway.auth.mode = "token"` in production
2. Configure signing secrets for all webhook channels
3. Restrict Telegram `allow_from` to known user IDs
4. Run behind a reverse proxy with TLS
5. Regularly rotate credentials and gateway tokens
6. Monitor `/api/metrics` for anomalous patterns
