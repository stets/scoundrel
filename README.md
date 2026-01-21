# Scoundrel

A terminal UI card game based on [Scoundrel](http://stfj.net/art/2011/Scoundrel.pdf) by Zach Gage and Kurt Bieg (2011).

![Scoundrel TUI](https://img.shields.io/badge/TUI-ratatui-blue)

## Installation

### From source

```bash
git clone https://github.com/stets/scoundrel.git
cd scoundrel
cargo build --release
./target/release/scoundrel
```

### With cargo

```bash
cargo install --git https://github.com/stets/scoundrel
```

## How to Play

You are a scoundrel delving into a dungeon. Survive by playing through all 44 cards.

### Card Types

| Card | Type | Effect |
|------|------|--------|
| Spades/Clubs | Monster | Deal damage equal to their value (2-14) |
| Diamonds | Weapon | Reduce monster damage by weapon value |
| Hearts | Potion | Restore health (max 20 HP) |

### Rules

- Each room has 4 cards - you must play exactly 3
- The 4th card stays for the next room
- You may skip a room (but not twice in a row)
- Only ONE potion heals per turn (second is wasted)
- Weapons degrade: after killing a monster, weapon can only hit monsters with LOWER value

### Controls

| Key | Action |
|-----|--------|
| Tab / Arrows | Navigate cards |
| Enter / Space | Play selected card |
| 1-4 | Play card by number |
| S | Skip room |
| L | View adventure log |
| ? | Help |
| Q | Quit |

## Credits

- Original game design: Zach Gage and Kurt Bieg
- TUI implementation: Built with [ratatui](https://github.com/ratatui-org/ratatui)

## License

MIT
