# Hands

For [Horizon](https://playhorizon.dev) (Minecraft: Java Edition 26.3 in Rust). Your hands keep up with you: hit a block and the right tool from your hotbar is in your hand, place your last block and the next stack is there, break your pickaxe and the next one takes its place. No keys, no screens, nothing to set up.

## How it plays
- **The right tool**: hit a block and the hotbar's fastest tool for it comes to your hand (pickaxe on stone, axe on wood, shovel on dirt). A tool with one use left is passed over.
- **Back to what you held**: a second after you stop digging, your hand goes back to the slot you had, so you can go on placing.
- **Refill**: when a hotbar stack runs out by use (placing, eating, throwing), the same item from your inventory fills the slot. Dropping it with Q doesn't.
- **A new tool**: when a tool breaks, one like it from your inventory takes its slot (the same tool first, else one of its kind).
- **Sneak** while hitting to keep what you hold.

Works for everyone, the host of a LAN world too. Nothing to earn.

## Install
1. Copy `hands.hmod` into `%APPDATA%\horizon\horizon-native-mods\`.
2. Add the line `mod.hands:true` to `%APPDATA%\horizon\horizon-mods.txt`.
3. Build: the launcher's Mods page **Build**, or `horizon-launcher native-build --instance horizon --trust-native-code`. A native mod runs with your rights: install it only from a source you trust.

The server needs the mod (in single player or LAN, the host's game is the server). Built for Horizon Nightly `f037744` (2026-11-21).

## Licence
MIT, see [LICENSE](LICENSE). Not affiliated with the Horizon project.
