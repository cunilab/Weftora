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

Export does not automatically copy Engine dependency notices. Before sharing an
export, create My Game\notices and copy this toolkit's LICENSE-MIT, LICENSE-APACHE,
THIRD-PARTY-NOTICES.txt, dependency-notices.json and complete licenses directory
into it. Keep your Story's font/media/content notices too. This toolkit inventory
is a conservative superset including authoring-only dependencies; copying notices
does not put the compiler in your game or complete the pending release audit.

For the opt-in developer tools:

  .\weftora.exe run "My Story" --debug-tools

Press F12; Tab cycles state, command and trace modes. State/command input uses
Ctrl+A to clear JSON and Enter to apply/run. Trace is read-only: Up/Down or
PageUp/PageDown scroll; Delete clears history. F12/Escape closes the panel.
Persistent edits/commands require a new authored checkpoint before saving.
To preview one authored node from defaults, append --start-node ExactTitle after
--debug-tools. Save/load are disabled for that preview; normal saves stay untouched.
Guide: https://github.com/cunilab/Weftora/blob/main/docs/debug-tools.md

For toolkit version and build-pair diagnostics:

  .\weftora.exe --version
  .\weftora.exe --tool-identity
  .\weftora-player.exe --tool-identity

Guide: https://github.com/cunilab/Weftora/blob/main/docs/getting-started.md
Issues: https://github.com/cunilab/Weftora/issues

Weftora is offered under MIT OR Apache-2.0; see LICENSE-MIT and LICENSE-APACHE.
Third-party components retain their own terms. THIRD-PARTY-NOTICES.txt lists
available upstream texts in licenses/; dependency-notices.json records hashes
and missing-text cases. THIRD-PARTY-NOTICES.txt also identifies included source
archives under licenses/ and their terms. Keep these archives when copying the
notice tree; included source archives can be extracted locally without a download.
The licence/advisory audit is pending: this verification
candidate must not be published as an approved release. Included licence texts
and metadata do not by themselves establish all redistribution obligations.
