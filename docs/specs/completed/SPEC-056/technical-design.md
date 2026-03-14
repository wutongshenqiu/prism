# Technical Design: Gemini Multimodal Enhancement

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-056       |
| Title     | Gemini Multimodal Enhancement |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Overview

Fixes image URL handling and adds PDF support for the Gemini translation path. Remote image URLs are properly converted to Gemini's fileData or inlineData format instead of being degraded to text.

## Backend Implementation

### Image URL Fix (`openai_to_gemini.rs`)

Replace text-fallback behavior with proper image handling:

```rust
fn convert_image_url(url: &str, mode: GeminiImageMode) -> GeminiPart {
    match mode {
        GeminiImageMode::FileData => {
            // Use Gemini's fileData with the URL directly
            json!({"fileData": {"mimeType": guess_mime(url), "fileUri": url}})
        }
        GeminiImageMode::Download => {
            // Download image, convert to base64 inlineData
            // Uses reqwest (already in deps)
            download_and_inline(url).await
        }
        GeminiImageMode::TextFallback => {
            // Current behavior - text reference
            json!({"text": format!("[image: {}]", url)})
        }
    }
}
```

### PDF Support

Handle document/file content parts:
- OpenAI `{"type": "file", "file": {"url": "data:application/pdf;base64,..."}}` → Gemini `inlineData`
- Claude `{"type": "document", "source": {...}}` → Gemini `inlineData`

## Configuration Changes

```yaml
gemini-image-mode: download  # file-data | download | text-fallback
```

Add `GeminiImageMode` enum to config.rs.

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| Gemini   | Yes       | Target for multimodal enhancement |
| OpenAI   | N/A       | Source format, not affected |
| Claude   | N/A       | Source format, not affected |

## Task Breakdown

- [ ] Add GeminiImageMode config
- [ ] Implement convert_image_url with fileData support
- [ ] Implement image download + base64 encoding
- [ ] Add PDF/document content part handling
- [ ] Add unit tests
- [ ] Add integration tests with mock images

## Test Strategy

- **Unit tests:** Image URL conversion, PDF base64 encoding, MIME type detection
- **Integration tests:** Full translation with image URLs
- **Manual verification:** Send image/PDF requests through proxy to Gemini

## Rollout Plan

1. Add config with `download` as default
2. Implement image URL handling
3. Add PDF support
