# Presentation demo

Run `weftora check examples/presentation-demo`, then `weftora run examples/presentation-demo`. Each Next/Space/background click starts the next stage. Wait for the stated transition duration before advancing, except the last voice/effect fade-out step, which deliberately interrupts the one-second tone. Volume opens a modal UI with Mute/50%/100% controls for master and every channel.

All PNGs are generated solid-color RGBA fixtures; both character images are rectangles. WAVs are generated quiet one-second sine tones at 220/330/440/660 Hz for music/ambience/voice/effects. These fixtures are released under CC0 and demonstrate operations, not final art or music. Reusing these tiny files makes the scripted demo reproducible.

Native checks and blank records are under issue #5 and `docs/evidence/phase-2-windows.md`. Headless mode validates and traces commands but does not animate, play sounds or run UI actions. The player binary is unchanged when this Story's dialogue, screens or assets are edited and re-exported.
