# Command Center OpenRouter routing and local cost history

Verified against OpenRouter's public documentation and unauthenticated catalog on **2026-11-28**.

## Pinned route

The default request uses:

```json
{
  "model": "deepseek/deepseek-v4.1-flash-1017",
  "response_format": {
    "type": "json_schema",
    "json_schema": {
      "name": "draft_proposal",
      "strict": true,
      "schema": { "...": "the Command Center proposal schema" }
    }
  },
  "max_tokens": 2000,
  "provider": {
    "order": ["sail-research/fp4"],
    "allow_fallbacks": false,
    "require_parameters": true
  }
}
```

`sail-research/fp4` is an **endpoint tag in `provider.order`**, not a model-ID suffix. The unauthenticated endpoint catalog lists that exact tag for the requested versioned model and advertises both `response_format` and `structured_outputs`. `require_parameters: true` makes a schema-capable endpoint mandatory; there is no JSON-only or alternate-model fallback. A response naming a different model is rejected, while any reported billed cost is still retained.

Official references:

- Provider routing and exact endpoint tags: <https://openrouter.ai/docs/guides/routing/provider-selection>
- Structured outputs: <https://openrouter.ai/docs/guides/features/structured-outputs>
- Model/endpoint catalog: <https://openrouter.ai/api/v1/models/deepseek/deepseek-v4.1-flash-1017/endpoints>
- Usage accounting: <https://openrouter.ai/docs/use-cases/usage-accounting>

## Request privacy

The backend sends only `Authorization: Bearer …`, `Content-Type: application/json`, and normal HTTP protocol headers. It sends no `HTTP-Referer`, title, app-attribution, analytics, cookie, member-identity, forwarding, or tracking headers. The body has no OpenRouter `metadata`, `user`, session, or analytics fields. Its user message is the existing functional allowlist: submitted text, local date, unsaved draft rows needed for stable follow-up references, and relevant account/category names. Stable draft row IDs are functional references, not analytics identifiers.

The existing `OPENROUTER_MODEL` and `OPENROUTER_URL` deployment overrides remain supported, and process environment values still win over Git-ignored `backend/.env`. The provider pin and fallback prohibition do not change. `PAISA_ASSISTANT_MODE=stub` remains the deterministic offline mode; `off` disables the assistant. The key is loaded normally by the backend and is never exposed to the frontend.

## Actual billed cost

OpenRouter reports the total charged amount in `usage.cost`. Paisa stores that value only; it never estimates cost from catalog prices or token counts. `serde_json` retains the numeric token text, and `rust_decimal` stores/sums base-10 values without an `f64` conversion (96-bit coefficient, up to 28 decimal places). API totals are decimal strings so the browser does not round them through JavaScript `Number`.

Migration `013_assistant_usage.sql` stores only:

- member scope, creation/completion timestamps, and attempt status;
- configured model and provider;
- a SHA-256 key for the client submission ID, for replay deduplication;
- an OpenRouter generation ID when safely recognized, for generation deduplication;
- whether cost was reported and the exact USD decimal when present.

It never stores prompts, raw responses, credentials, request/response headers, or external tracking metadata. Pending and unavailable attempts support idempotency but are not cost-history entries. Reusing a generation ID marks the later attempt as a duplicate and excludes it from totals.

A successful response is recorded before proposal validation, so a billed response with invalid proposal JSON still contributes its reported cost. An error response with a `usage` object is handled the same way. An explicitly reported `0` is a reported cost; an absent, malformed, negative, or unrepresentable cost is counted as missing, never as zero.

## Read-only API and UI

`GET /api/assistant/usage` resolves the member from the existing trusted identity extractor. It accepts no member query parameter and has no write method. Household callers see only their own records.

```json
{
  "member_id": 1,
  "total_cost_usd": "0.300000000000000001",
  "completed_count": 6,
  "reported_count": 4,
  "zero_cost_count": 1,
  "missing_cost_count": 2,
  "days": [
    {
      "date": "2026-11-01",
      "total_cost_usd": "0.3",
      "completed_count": 2,
      "reported_count": 2,
      "zero_cost_count": 0,
      "missing_cost_count": 0
    }
  ]
}
```

Command Center shows a collapsed **Assistant cost history** section with the exact all-time USD total and per-day totals/counts. It is operational information, not a spending dashboard, and it never converts ledger activity into assistant cost.
