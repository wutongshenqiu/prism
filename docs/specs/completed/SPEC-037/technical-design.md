# SPEC-037: Technical Design — Split dispatch.rs

## Proposed Structure

```
crates/server/src/dispatch/
  mod.rs              # Main dispatch() + DispatchRequest/DispatchMeta (orchestrator)
  helpers.rs          # extract_usage(), inject_dispatch_meta(), inject_debug_headers(),
                      # rewrite_model_in_body(), build_json_response()
  streaming.rs        # translate_stream(), build_keepalive_body()
  retry.rs            # handle_retry_error(), retry-related constants
```

## Migration Steps

1. Create `dispatch/` directory
2. Move `dispatch.rs` to `dispatch/mod.rs`
3. Extract helper functions to `helpers.rs` (pure functions, no state)
4. Extract streaming helpers to `streaming.rs`
5. Extract retry helpers to `retry.rs`
6. Update `mod.rs` to re-export public types (`DispatchMeta`, `DispatchRequest`, `dispatch()`)
7. Verify all tests pass

## Key Constraint

All functions currently in `dispatch.rs` tests must continue to work. The test module stays in `mod.rs` importing from sub-modules.
