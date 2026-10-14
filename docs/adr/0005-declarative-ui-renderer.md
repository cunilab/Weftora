---
id: WFT-ADR-005
title: ADR 0005 — Declarative UI Renderer
status: proposed
---

# ADR 0005 — Declarative UI Renderer

## Context

Phase 2 needs a renderer for Story-defined screen, panel, text, image, button, list, choice-list and overlay widgets. The current presentation adapter already draws dialogue and choices with Bevy UI. Replacing it would add another rendering/input integration before the first playable visual novel.

## Decision: Bevy UI inside the existing adapter

Use Bevy UI, pinned to `=0.20.0`, inside `weftora-render-bevy`. Keep screen schemas and validation in the Bevy-free Story layer, canonical bindings in the Engine state contract, and runtime drawing/input translation in the adapter. Do not add another UI dependency or split out a separate UI crate until the code needs that boundary.

Story authors choose content and layout through JSONC. They do not name Bevy components or wire systems. Image, audio and font files resolve through Story asset IDs and its path policy; the adapter reads bounded bytes itself rather than letting AssetServer open Story paths. Legacy assets remain untyped and are verified by the relevant decoder.

Bindings update after dialogue/command state changes at a deterministic presentation boundary. Widget/action/binding validation belongs in `weftora check`. Keyboard focus, mouse activation, resizing and font rendering must be verified on Windows before P2-06 passes. This ADR chooses the renderer; it does not claim those behaviors already work.

## Consequences

- Reuses the existing Node/ImageNode/Text/Button/Activate integration and its error screen.
- Bevy widget/focus API changes stay inside the adapter and require dependency/Windows regression checks on upgrades.
- Phase 2 still needs the declarative schema, real canonical-state bindings, focus handling and a Windows performance baseline. Revisit the renderer if the baseline or Windows evidence fails the agreed budgets.
