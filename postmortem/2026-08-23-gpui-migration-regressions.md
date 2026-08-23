# GPUI migration regressions

## What happened

The migration from the previous renderer to GPUI introduced user-visible
regressions, including startup crashes, broken preview navigation and layout,
and failed remote-image loading.

## Root cause

The renderer migration crossed several platform boundaries at once: GPUI
layout and scroll semantics, asset loading, native window behavior, and HTTP
proxy handling. The initial remote-image recovery path also returned before
its macOS network fallback could run when a stale localhost proxy failed.

## Fix applied

The preview now uses stable native image caching, explicit background loading,
and a macOS direct-download fallback that bypasses stale loopback proxy
settings. The surrounding migration restores document styling, HTML/MDX
blocks, Mermaid sizing, navigation, settings, icons, and responsive preview
layout.

## What we learned

Renderer migrations require end-to-end checks in the packaged application,
not only unit tests or a successful build. Network loading tests now cover the
redirecting image URL and the macOS fallback path; failures retain their
underlying network reason for diagnosis.
