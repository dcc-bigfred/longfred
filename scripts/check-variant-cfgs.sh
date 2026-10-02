#!/usr/bin/env bash
# Fail if a Cargo variant feature name appears outside board/variants.
#
# Hardware selection is `cfg(feature = "variant-…")` only in
# crates/firmware/src/board/variants/. Callers ask VariantDescriptor or
# board::spawn_inputs. See ARCHITECTURE.md §1.6 and §10.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

hits="$(grep -RIn --exclude-dir=variants 'variant-' crates/firmware/src || true)"
if [[ -n "$hits" ]]; then
    printf '%s\n' "$hits" >&2
    echo "error: \`variant-\` outside crates/firmware/src/board/variants/" >&2
    exit 1
fi
