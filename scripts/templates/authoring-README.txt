Weftora authoring tools — verification candidate

Keep weftora.exe and weftora-player.exe together. This Windows toolkit does not
need Rust or Cargo. In PowerShell, open this folder and try:

  .\weftora.exe new "My Story"
  .\weftora.exe check "My Story"
  .\weftora.exe run "My Story"
  .\weftora.exe export "My Story" --out "My Game"
  & ".\My Game\weftora-player.exe" ".\My Game\story"

Edit My Story\main.yarn and check again. Use a fresh export destination each time.
Keep every file in the exported game folder together when sharing your game.
The CLI must stay beside its matching player when creating another export.

For the opt-in developer state inspector:

  .\weftora.exe run "My Story" --debug-tools

Press F12; use Up/Down to select, Ctrl+A to replace JSON, Enter to apply, and
F12/Escape to close. Persistent edits require a new authored checkpoint before
saving. Guide: https://github.com/cunilab/Weftora/blob/main/docs/debug-tools.md

For toolkit version and build-pair diagnostics:

  .\weftora.exe --version
  .\weftora.exe --tool-identity
  .\weftora-player.exe --tool-identity

Guide: https://github.com/cunilab/Weftora/blob/main/docs/getting-started.md
Issues: https://github.com/cunilab/Weftora/issues

Weftora is offered under MIT OR Apache-2.0; see LICENSE-MIT and LICENSE-APACHE.
Third-party components retain their own terms. THIRD-PARTY-NOTICES.txt lists
available upstream texts in licenses/; dependency-notices.json records hashes
and missing-text cases. The licence/advisory audit is pending: this verification
candidate must not be published as an approved release. Included licence texts
and metadata do not by themselves establish all redistribution obligations.
