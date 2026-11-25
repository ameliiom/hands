# Hands: design

Your hands keep up with you: hit a block and the right tool from your hotbar is in your hand,
place your last block and the next stack is there, break your pickaxe and the next one takes
its place. No keys, no screens, nothing to set up.

## 1. Outcome

No more scrolling the hotbar between stone and dirt, no more opening the inventory mid-build to
fetch the next stack, no more digging on with your bare hand after a tool breaks.

## 2. Prior art (searched 2026-11-25)

| What | Where | Kind | Use |
|---|---|---|---|
| Horizon mods (GitHub `horizon-mod.toml`, 38 repos) | GitHub | no tool swap, refill or tool restock | build ours |
| AutoSwitch | Java (Fabric) | the best tool on attack, switch back after, per-tool rules | design only |
| Inventory Profiles Next | Java | hotbar refill, tool restock before it breaks, sorting | design only |
| Mouse Tweaks | Java | inventory drag and wheel gestures | out of scope (Storage has its own) |

(Also found while searching: two Horizon vein miners, VeinSweep and veinminer-horizon, published
after Gathering's search; Gathering stays ours, with trees, harvest and the Skills tie.)

## 3. How it plays

- **The right tool**: hit a block and the hotbar's fastest tool for it comes to your hand
  (pickaxe on stone, axe on wood, shovel on dirt, shears on leaves). A tool that would break on
  this block (one use left) is passed over.
- **Back to what you held**: a second after you stop digging, the hand goes back to the slot you
  had (so you can go on placing the block you held).
- **Refill**: when a hotbar stack runs out by use (placing, eating, throwing a pearl), the same
  item from your inventory fills the slot. Dropping it (Q) or putting it away doesn't.
- **A new tool**: when your tool breaks, one of the same kind from your inventory takes its slot
  (a diamond pickaxe for a diamond pickaxe, else another pickaxe).
- **Sneak** while hitting: no swap.

## 4. Choices

| Question | Picked | Over | Why |
|---|---|---|---|
| Who swaps the tool | the server, on the block's first hit (`left_click_block` → `set_selected_slot`) | the client before the hit | the client can't read its hotbar through the API; the server costs 1 to 6 ticks once (measured) |
| How a run-out is seen | the hand compared tick to tick | the tool-break hook | the hook is not heard for the LAN host (you), the comparison is (measured) |
| Skills | free for everyone | a Skills tree | small comforts the game should have had; nothing to earn |

## 5. Measured (owner's PC, Horizon f037744, a spike build, 2026-11-25)

| Case | Result |
|---|---|
| Stone, diamond pickaxe already in hand | broken in 6 ticks |
| Stone, dirt in hand, pickaxe in the hotbar | swapped on the first hit (speed 1 → 8), broken in **12 ticks** (6 more, once) instead of 150 by hand |
| Oak log, dirt in hand, axe in the hotbar | swapped, **14 ticks** against 13 with the axe already in hand |
| Last cobblestone placed, 64 in the inventory | the slot held 64 again the next tick, the inventory slot empty |
| Diamond pickaxe broken on its last use (single player host) | a spare diamond pickaxe from the inventory was in the slot the next tick |
| Clearing the inventory with a command | the spike also "refilled" (nothing there): the build tells use from drop and clear |

## 6. The skeleton (0.1.0), measured in the game (2026-11-25)

The swap, the refill and the new tool, with the decisions on plain values (`rules.rs`, tested)
and the server hooks (`left_click_block`, `item_toss`, the tick's hand comparison).

| Case | Result |
|---|---|
| Dirt in hand, diamond pickaxe in the hotbar, stone | swapped, broken in 8 ticks |
| The same while sneaking | no swap: dirt stays in hand |
| Diamond pickaxe with one use left and a stone pickaxe in the hotbar | the stone pickaxe was picked (14 ticks), the worn one kept |
| Last cobblestone placed, 64 in the inventory | slot refilled with 64, the inventory slot emptied |
| Last cobblestone dropped with Q | no refill: the 64 stayed in the inventory |
| Iron pickaxe broken, a stone pickaxe in the inventory | the stone pickaxe took its slot (same kind) |

Still to build: back to the held slot a second after digging stops.

## 7. Risks and answers

| Risk | Answer |
|---|---|
| Swapping while you meant to hit with a sword | only blocks swap; hitting mobs never does |
| The swap restarts the dig | measured 1 to 6 ticks once per block, then normal |
| Refill on a drop | drops (`item_toss`) and inventory moves are told apart from use |
| A favourite tool used up | a tool with one use left is never picked, and a broken one is replaced only by its own kind |
| Silk Touch or Fortune wanted on purpose | the hand only changes when the held item is slower; a tool you chose that mines the block stays |

## 8. Phases

1. The right tool, back to what you held, refill, a new tool (the whole mod).
