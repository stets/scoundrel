use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use rand::seq::SliceRandom;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, BorderType, Clear, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Suit {
    Spades,
    Clubs,
    Hearts,
    Diamonds,
}

impl Suit {
    fn symbol(&self) -> &str {
        match self {
            Suit::Spades => "♠",
            Suit::Clubs => "♣",
            Suit::Hearts => "♥",
            Suit::Diamonds => "♦",
        }
    }

    fn color(&self) -> Color {
        match self {
            Suit::Hearts | Suit::Diamonds => Color::Red,
            _ => Color::White,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Card {
    suit: Suit,
    rank: u8, // 2-14 (11=J, 12=Q, 13=K, 14=A)
}

impl Card {
    fn rank_str(&self) -> String {
        match self.rank {
            11 => "J".to_string(),
            12 => "Q".to_string(),
            13 => "K".to_string(),
            14 => "A".to_string(),
            n => n.to_string(),
        }
    }

    fn display(&self) -> String {
        format!("{}{}", self.rank_str(), self.suit.symbol())
    }

    fn is_monster(&self) -> bool {
        matches!(self.suit, Suit::Spades | Suit::Clubs)
    }

    fn is_weapon(&self) -> bool {
        matches!(self.suit, Suit::Diamonds)
    }

    fn is_potion(&self) -> bool {
        matches!(self.suit, Suit::Hearts)
    }

    fn value(&self) -> u8 {
        self.rank
    }

    fn type_emoji(&self) -> &str {
        if self.is_monster() {
            "👹"
        } else if self.is_weapon() {
            "⚔️"
        } else {
            "🧪"
        }
    }

    fn type_str(&self) -> String {
        if self.is_monster() {
            format!("Take {} damage", self.value())
        } else if self.is_weapon() {
            format!("{} attack power", self.value())
        } else {
            format!("Heal {} HP", self.value())
        }
    }

    fn type_label(&self) -> &str {
        if self.is_monster() {
            "MONSTER"
        } else if self.is_weapon() {
            "WEAPON"
        } else {
            "POTION"
        }
    }
}

#[derive(Clone)]
struct Weapon {
    card: Card,
    last_monster_slain: Option<u8>,
}

impl Weapon {
    fn can_use_against(&self, monster_value: u8) -> bool {
        match self.last_monster_slain {
            None => true,
            Some(last) => monster_value < last,  // Strictly less than, weapon degrades
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Screen {
    Game,
    Combat,
    Help,
    Log,
    GameOver,
    ConfirmQuit,
}

struct GameState {
    dungeon: Vec<Card>,
    room: Vec<Card>,
    discard: Vec<Card>,
    health: i32,
    max_health: i32,
    weapon: Option<Weapon>,
    monsters_on_weapon: Vec<Card>,
    cards_played_this_turn: u8,
    potion_used_this_turn: bool,
    just_skipped: bool,
    game_over: bool,
    won: bool,
    last_card_was_potion: Option<Card>,
    log: Vec<String>,
    turn_number: u32,
    selected_index: usize,
    screen: Screen,
    combat_card_index: Option<usize>,
    combat_selection: usize, // 0 = weapon, 1 = barehanded, 2 = back
    message: String,
}

impl GameState {
    fn new() -> Self {
        let mut state = GameState {
            dungeon: Vec::new(),
            room: Vec::new(),
            discard: Vec::new(),
            health: 20,
            max_health: 20,
            weapon: None,
            monsters_on_weapon: Vec::new(),
            cards_played_this_turn: 0,
            potion_used_this_turn: false,
            just_skipped: false,
            game_over: false,
            won: false,
            last_card_was_potion: None,
            log: Vec::new(),
            turn_number: 1,
            selected_index: 0,
            screen: Screen::Game,
            combat_card_index: None,
            combat_selection: 0,
            message: String::new(),
        };
        state.setup_deck();
        state.log("Entered the dungeon with 20 HP".to_string());
        state.deal_room();
        state
    }

    fn log(&mut self, msg: String) {
        self.log.push(format!("[Turn {}] {}", self.turn_number, msg));
    }

    fn setup_deck(&mut self) {
        self.dungeon.clear();
        // Black suits: full range 2-14
        for suit in [Suit::Spades, Suit::Clubs] {
            for rank in 2..=14 {
                self.dungeon.push(Card { suit, rank });
            }
        }
        // Red suits: only 2-10 (no face cards or aces)
        for suit in [Suit::Hearts, Suit::Diamonds] {
            for rank in 2..=10 {
                self.dungeon.push(Card { suit, rank });
            }
        }
        let mut rng = rand::thread_rng();
        self.dungeon.shuffle(&mut rng);
    }

    fn deal_room(&mut self) {
        while self.room.len() < 4 && !self.dungeon.is_empty() {
            self.room.push(self.dungeon.remove(0));
        }
        self.cards_played_this_turn = 0;
        self.potion_used_this_turn = false;
        self.last_card_was_potion = None;
        self.selected_index = 0;

        if !self.room.is_empty() {
            let room_str: Vec<String> = self.room.iter().map(|c| c.display()).collect();
            self.log(format!("Entered room: {}", room_str.join(", ")));
        }
    }

    fn play_potion(&mut self, index: usize) {
        let card = self.room.remove(index);

        if self.potion_used_this_turn {
            self.message = format!("Second potion - {} wasted!", card.display());
            self.log(format!("Wasted {} (already used potion)", card.display()));
        } else {
            let heal = (card.value() as i32).min(self.max_health - self.health);
            self.health += heal;
            self.potion_used_this_turn = true;
            self.last_card_was_potion = Some(card);
            self.message = format!("Used {} - healed {} HP!", card.display(), heal);
            self.log(format!(
                "Drank {}, healed {} HP (now {} HP)",
                card.display(),
                heal,
                self.health
            ));
        }

        self.discard.push(card);
        self.cards_played_this_turn += 1;
        self.check_turn_complete();
    }

    fn play_weapon(&mut self, index: usize) {
        let card = self.room.remove(index);

        if let Some(ref old_weapon) = self.weapon {
            let old = old_weapon.card.display();
            self.discard.push(old_weapon.card);
            self.discard.extend(self.monsters_on_weapon.drain(..));
            self.log(format!("Discarded {}, equipped {}", old, card.display()));
        } else {
            self.log(format!("Equipped {}", card.display()));
        }

        self.weapon = Some(Weapon {
            card,
            last_monster_slain: None,
        });
        self.last_card_was_potion = None;
        self.message = format!("Equipped {}!", card.display());

        self.cards_played_this_turn += 1;
        self.check_turn_complete();
    }

    fn can_use_weapon_on(&self, card: &Card) -> bool {
        if let Some(ref weapon) = self.weapon {
            weapon.can_use_against(card.value())
        } else {
            false
        }
    }

    fn fight_monster(&mut self, index: usize, use_weapon: bool) {
        let card = self.room.remove(index);

        let damage = if use_weapon {
            let weapon = self.weapon.as_mut().unwrap();
            let dmg = (card.value() as i32 - weapon.card.value() as i32).max(0);
            weapon.last_monster_slain = Some(card.value());
            let weapon_display = weapon.card.display();
            let card_display = card.display();
            self.monsters_on_weapon.push(card);
            self.message = format!("Slew {} with weapon - took {} damage!", card_display, dmg);
            self.log(format!(
                "Killed {} with {}, took {} dmg (now {} HP)",
                card_display,
                weapon_display,
                dmg,
                self.health - dmg
            ));
            dmg
        } else {
            let dmg = card.value() as i32;
            self.discard.push(card);
            self.message = format!("Fought {} barehanded - took {} damage!", card.display(), dmg);
            self.log(format!(
                "Fought {} barehanded, took {} dmg (now {} HP)",
                card.display(),
                dmg,
                self.health - dmg
            ));
            dmg
        };

        self.health -= damage;
        self.last_card_was_potion = None;
        self.cards_played_this_turn += 1;

        if self.health <= 0 {
            self.health = 0;
            self.game_over = true;
            self.won = false;
            self.log("DIED!".to_string());
            self.screen = Screen::GameOver;
        } else {
            self.check_turn_complete();
        }
    }

    fn check_turn_complete(&mut self) {
        if self.cards_played_this_turn >= 3 {
            self.turn_number += 1;

            if self.dungeon.is_empty() && self.room.len() == 1 {
                // Must play final card
                self.message = "Final card! You must face it.".to_string();
                self.cards_played_this_turn = 0;
                self.potion_used_this_turn = false;
                self.selected_index = 0;
            } else if self.dungeon.is_empty() && self.room.is_empty() {
                self.game_over = true;
                self.won = true;
                self.log(format!("VICTORY! Score: {}", self.calculate_score()));
                self.screen = Screen::GameOver;
            } else {
                self.just_skipped = false;
                self.deal_room();
            }
        }

        if self.selected_index >= self.room.len() && !self.room.is_empty() {
            self.selected_index = self.room.len() - 1;
        }
    }

    fn skip_room(&mut self) {
        if self.just_skipped {
            self.message = "Cannot skip two rooms in a row!".to_string();
            return;
        }
        if self.cards_played_this_turn > 0 {
            self.message = "Cannot skip after playing cards!".to_string();
            return;
        }

        let room_str: Vec<String> = self.room.iter().map(|c| c.display()).collect();
        self.dungeon.extend(self.room.drain(..));
        self.just_skipped = true;
        self.log(format!("Skipped room ({})", room_str.join(", ")));
        self.message = "Skipped room".to_string();
        self.deal_room();
    }

    fn calculate_score(&self) -> i32 {
        if self.won {
            let mut score = self.health;
            if self.health == self.max_health {
                if let Some(ref potion) = self.last_card_was_potion {
                    score += potion.value() as i32;
                }
            }
            score
        } else {
            let remaining: i32 = self
                .dungeon
                .iter()
                .chain(self.room.iter())
                .filter(|c| c.is_monster())
                .map(|c| c.value() as i32)
                .sum();
            self.health - remaining
        }
    }

    fn reset(&mut self) {
        *self = GameState::new();
    }
}

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut game = GameState::new();
    let result = run_app(&mut terminal, &mut game);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    game: &mut GameState,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, game))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match game.screen {
                Screen::Game => match key.code {
                    KeyCode::Char('q') => game.screen = Screen::ConfirmQuit,
                    KeyCode::Char('?') => game.screen = Screen::Help,
                    KeyCode::Char('l') => game.screen = Screen::Log,
                    KeyCode::Char('s') => game.skip_room(),
                    KeyCode::Tab | KeyCode::Right => {
                        if !game.room.is_empty() {
                            game.selected_index = (game.selected_index + 1) % game.room.len();
                        }
                    }
                    KeyCode::BackTab | KeyCode::Left => {
                        if !game.room.is_empty() {
                            game.selected_index = if game.selected_index == 0 {
                                game.room.len() - 1
                            } else {
                                game.selected_index - 1
                            };
                        }
                    }
                    KeyCode::Down => {
                        if game.selected_index + 2 < game.room.len() {
                            game.selected_index += 2;
                        }
                    }
                    KeyCode::Up => {
                        if game.selected_index >= 2 {
                            game.selected_index -= 2;
                        }
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if game.selected_index < game.room.len() {
                            let card = &game.room[game.selected_index];
                            if card.is_potion() {
                                game.play_potion(game.selected_index);
                            } else if card.is_weapon() {
                                game.play_weapon(game.selected_index);
                            } else {
                                // Monster - if no weapon, attack directly
                                if game.weapon.is_none() {
                                    game.fight_monster(game.selected_index, false);
                                } else {
                                    // Has weapon - show combat options
                                    game.combat_card_index = Some(game.selected_index);
                                    game.combat_selection = 0;
                                    game.screen = Screen::Combat;
                                }
                            }
                        }
                    }
                    KeyCode::Char(c) if c >= '1' && c <= '4' => {
                        let idx = (c as usize) - ('1' as usize);
                        if idx < game.room.len() {
                            game.selected_index = idx;
                            let card = &game.room[idx];
                            if card.is_potion() {
                                game.play_potion(idx);
                            } else if card.is_weapon() {
                                game.play_weapon(idx);
                            } else {
                                // Monster - if no weapon, attack directly
                                if game.weapon.is_none() {
                                    game.fight_monster(idx, false);
                                } else {
                                    game.combat_card_index = Some(idx);
                                    game.combat_selection = 0;
                                    game.screen = Screen::Combat;
                                }
                            }
                        }
                    }
                    _ => {}
                },
                Screen::Combat => {
                    let card_idx = game.combat_card_index.unwrap();
                    let card = &game.room[card_idx];
                    let can_use_weapon = game.can_use_weapon_on(card);
                    let num_options = if can_use_weapon { 3 } else { 2 };

                    match key.code {
                        KeyCode::Up | KeyCode::BackTab => {
                            game.combat_selection = if game.combat_selection == 0 {
                                num_options - 1
                            } else {
                                game.combat_selection - 1
                            };
                        }
                        KeyCode::Down | KeyCode::Tab => {
                            game.combat_selection = (game.combat_selection + 1) % num_options;
                        }
                        KeyCode::Enter | KeyCode::Char(' ') => {
                            if can_use_weapon {
                                match game.combat_selection {
                                    0 => {
                                        game.fight_monster(card_idx, true);
                                        game.screen = Screen::Game;
                                    }
                                    1 => {
                                        game.fight_monster(card_idx, false);
                                        game.screen = Screen::Game;
                                    }
                                    _ => game.screen = Screen::Game,
                                }
                            } else {
                                match game.combat_selection {
                                    0 => {
                                        game.fight_monster(card_idx, false);
                                        game.screen = Screen::Game;
                                    }
                                    _ => game.screen = Screen::Game,
                                }
                            }
                            game.combat_card_index = None;
                        }
                        KeyCode::Char('1') => {
                            if can_use_weapon {
                                game.fight_monster(card_idx, true);
                            } else {
                                game.fight_monster(card_idx, false);
                            }
                            game.screen = Screen::Game;
                            game.combat_card_index = None;
                        }
                        KeyCode::Char('2') if can_use_weapon => {
                            game.fight_monster(card_idx, false);
                            game.screen = Screen::Game;
                            game.combat_card_index = None;
                        }
                        KeyCode::Char('b') | KeyCode::Esc => {
                            game.screen = Screen::Game;
                            game.combat_card_index = None;
                        }
                        _ => {}
                    }
                }
                Screen::Help => {
                    game.screen = Screen::Game;
                }
                Screen::Log => {
                    game.screen = Screen::Game;
                }
                Screen::GameOver => match key.code {
                    KeyCode::Char('y') | KeyCode::Enter => {
                        game.reset();
                    }
                    KeyCode::Char('n') | KeyCode::Char('q') | KeyCode::Esc => {
                        return Ok(());
                    }
                    _ => {}
                },
                Screen::ConfirmQuit => match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        return Ok(());
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        game.screen = Screen::Game;
                    }
                    _ => {}
                },
            }
        }
    }
}

fn ui(f: &mut Frame, game: &GameState) {
    let size = f.area();

    // Main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Length(5),  // Stats
            Constraint::Length(1),  // Slain
            Constraint::Length(1),  // Room label
            Constraint::Min(14),    // Cards (bigger)
            Constraint::Length(2),  // Card info
            Constraint::Length(1),  // Controls
            Constraint::Length(1),  // Message
        ])
        .split(size);

    // Title
    let title = Paragraph::new("🏰 SCOUNDREL 🏰")
        .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded));
    f.render_widget(title, chunks[0]);

    // Stats row
    let stats_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(chunks[1]);

    // Health - vertically centered
    let health_pct = game.health as f32 / game.max_health as f32;
    let (health_color, health_emoji) = if health_pct > 0.5 {
        (Color::Green, "💚")
    } else if health_pct > 0.25 {
        (Color::Yellow, "💛")
    } else {
        (Color::Red, "❤️")
    };
    let bar_width = 10;
    let filled = (health_pct * bar_width as f32) as usize;
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(bar_width - filled));
    let health_text = format!("{} {}/{}\n{}", health_emoji, game.health, game.max_health, bar);
    let health = Paragraph::new(health_text)
        .style(Style::default().fg(health_color))
        .alignment(Alignment::Center)
        .block(Block::default().title(" HP ").borders(Borders::ALL).border_style(Style::default().fg(health_color)));
    f.render_widget(health, stats_chunks[0]);

    // Weapon
    let (weapon_text, weapon_color) = if let Some(ref w) = game.weapon {
        let durability = if let Some(last) = w.last_monster_slain {
            if last <= 2 {
                "Broken".to_string()
            } else {
                format!("Hits up to {}", last - 1)
            }
        } else {
            "Full".to_string()
        };
        (format!("⚔️ {}\n{}", w.card.display(), durability), Color::Yellow)
    } else {
        ("None\nunarmed".to_string(), Color::DarkGray)
    };
    let weapon = Paragraph::new(weapon_text)
        .style(Style::default().fg(weapon_color))
        .alignment(Alignment::Center)
        .block(Block::default().title(" Weapon ").borders(Borders::ALL).border_style(Style::default().fg(weapon_color)));
    f.render_widget(weapon, stats_chunks[1]);

    // Dungeon
    let dungeon_text = format!("🏰 {}\ncards left", game.dungeon.len());
    let dungeon = Paragraph::new(dungeon_text)
        .style(Style::default().fg(Color::Blue))
        .alignment(Alignment::Center)
        .block(Block::default().title(" Dungeon ").borders(Borders::ALL).border_style(Style::default().fg(Color::Blue)));
    f.render_widget(dungeon, stats_chunks[2]);

    // Turn
    let remaining = 3 - game.cards_played_this_turn;
    let pips = format!("{}{}", "● ".repeat(remaining as usize), "○ ".repeat(game.cards_played_this_turn as usize));
    let potion_status = if game.potion_used_this_turn {
        "🧪 used"
    } else {
        "play cards"
    };
    let turn_text = format!("{}\n{}", pips, potion_status);
    let turn = Paragraph::new(turn_text)
        .style(Style::default().fg(Color::Magenta))
        .alignment(Alignment::Center)
        .block(Block::default().title(" Turn ").borders(Borders::ALL).border_style(Style::default().fg(Color::Magenta)));
    f.render_widget(turn, stats_chunks[3]);

    // Slain monsters
    let slain_text = if !game.monsters_on_weapon.is_empty() {
        let slain: Vec<String> = game.monsters_on_weapon.iter().map(|c| c.display()).collect();
        format!("☠️ Slain: {}", slain.join(", "))
    } else {
        String::new()
    };
    let slain = Paragraph::new(slain_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(slain, chunks[2]);

    // Room label
    let room_label = Paragraph::new("THE ROOM")
        .style(Style::default().add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center);
    f.render_widget(room_label, chunks[3]);

    // Cards - 2x2 grid
    let cards_area = chunks[4];
    let card_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(cards_area);

    for (row_idx, row_area) in card_rows.iter().enumerate() {
        let cards_in_row: Vec<usize> = (0..game.room.len())
            .filter(|&i| i / 2 == row_idx)
            .collect();

        if cards_in_row.is_empty() {
            continue;
        }

        let card_constraints: Vec<Constraint> = cards_in_row
            .iter()
            .map(|_| Constraint::Length(22))
            .collect();

        // Center the cards
        let total_width: u16 = card_constraints.len() as u16 * 22 + (card_constraints.len() as u16 - 1) * 2;
        let padding = (row_area.width.saturating_sub(total_width)) / 2;

        let centered_area = Rect {
            x: row_area.x + padding,
            y: row_area.y,
            width: total_width.min(row_area.width),
            height: row_area.height,
        };

        let card_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(card_constraints)
            .split(centered_area);

        for (area_idx, &card_idx) in cards_in_row.iter().enumerate() {
            if card_idx < game.room.len() {
                let card = &game.room[card_idx];
                let is_selected = card_idx == game.selected_index;

                let (border_color, border_type) = if is_selected {
                    (Color::Cyan, BorderType::Double)
                } else {
                    (Color::White, BorderType::Rounded)
                };

                // Bigger, clearer card display
                let rank_display = card.rank_str();
                let big_rank = if rank_display.len() == 1 {
                    format!(" {} ", rank_display)
                } else {
                    format!("{} ", rank_display)
                };
                let card_content = format!(
                    "{} {}\n\n{}{}\n\n{}\n[{}]",
                    card.type_emoji(),
                    card.type_label(),
                    big_rank,
                    card.suit.symbol(),
                    card.type_str(),
                    card_idx + 1
                );

                let style = if is_selected {
                    Style::default().fg(card.suit.color()).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(card.suit.color())
                };

                let card_widget = Paragraph::new(card_content)
                    .style(style)
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(border_type)
                            .border_style(Style::default().fg(border_color)),
                    );

                f.render_widget(card_widget, card_areas[area_idx]);
            }
        }
    }

    // Card info
    let info_text = if !game.room.is_empty() && game.selected_index < game.room.len() {
        let card = &game.room[game.selected_index];
        if card.is_monster() {
            if game.can_use_weapon_on(card) {
                let wpn = game.weapon.as_ref().unwrap();
                let wpn_dmg = (card.value() as i32 - wpn.card.value() as i32).max(0);
                format!("▶ {} │ {} dmg barehanded, {} with weapon", card.display(), card.value(), wpn_dmg)
            } else {
                format!("▶ {} │ {} damage", card.display(), card.value())
            }
        } else if card.is_weapon() {
            format!("▶ {} │ equip for {} attack power", card.display(), card.value())
        } else {
            let heal = (card.value() as i32).min(game.max_health - game.health);
            if game.potion_used_this_turn {
                format!("▶ {} │ wasted - already used potion", card.display())
            } else {
                format!("▶ {} │ heal {} HP", card.display(), heal)
            }
        }
    } else {
        String::new()
    };
    let info = Paragraph::new(info_text)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center);
    f.render_widget(info, chunks[5]);

    // Controls
    let controls_text = "Tab/Arrows: move │ Enter: play │ S: skip │ L: log │ ?: help │ Q: quit";
    let controls = Paragraph::new(controls_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(controls, chunks[6]);

    // Message
    let msg = Paragraph::new(game.message.as_str())
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center);
    f.render_widget(msg, chunks[7]);

    // Modal screens
    match game.screen {
        Screen::Combat => render_combat_modal(f, game),
        Screen::Help => render_help_modal(f),
        Screen::Log => render_log_modal(f, game),
        Screen::GameOver => render_gameover_modal(f, game),
        Screen::ConfirmQuit => render_quit_modal(f),
        _ => {}
    }
}

fn render_combat_modal(f: &mut Frame, game: &GameState) {
    let area = centered_rect(50, 40, f.area());
    f.render_widget(Clear, area);

    let card_idx = game.combat_card_index.unwrap();
    let card = &game.room[card_idx];
    let can_use_weapon = game.can_use_weapon_on(card);

    let mut lines = vec![
        Line::from(Span::styled(
            format!("⚔️  Fighting {} (damage: {})", card.display(), card.value()),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if can_use_weapon {
        let wpn = game.weapon.as_ref().unwrap();
        let wpn_dmg = (card.value() as i32 - wpn.card.value() as i32).max(0);

        let style_0 = if game.combat_selection == 0 {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let style_1 = if game.combat_selection == 1 {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let style_2 = if game.combat_selection == 2 {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        lines.push(Line::from(Span::styled(
            format!("[1] 🗡️  Use weapon ({}) - take {} damage", wpn.card.display(), wpn_dmg),
            style_0,
        )));
        lines.push(Line::from(Span::styled(
            format!("[2] 👊 Fight barehanded - take {} damage", card.value()),
            style_1,
        )));
        lines.push(Line::from(Span::styled("[B] ← Back", style_2)));
    } else {
        if game.weapon.is_some() {
            let wpn = game.weapon.as_ref().unwrap();
            let max_can_hit = wpn.last_monster_slain.unwrap() - 1;
            lines.push(Line::from(Span::styled(
                format!("Weapon only hits up to {} (monster is {})", max_can_hit, card.value()),
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));
        }

        let style_0 = if game.combat_selection == 0 {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let style_1 = if game.combat_selection == 1 {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        lines.push(Line::from(Span::styled(
            format!("[1] 👊 Fight barehanded - take {} damage", card.value()),
            style_0,
        )));
        lines.push(Line::from(Span::styled("[B] ← Back", style_1)));
    }

    let combat = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .title("Combat")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(combat, area);
}

fn render_help_modal(f: &mut Frame) {
    let area = centered_rect(70, 80, f.area());
    f.render_widget(Clear, area);

    let help_text = r#"SCOUNDREL RULES
By Zach Gage and Kurt Bieg (2011)

GOAL
Survive the dungeon by playing through all 44 cards.

CARD TYPES
  ♠ ♣ Monsters  Deal damage equal to their value (2-14)
  ♦ Weapons     Reduce monster damage by weapon value
  ♥ Potions     Restore health (max 20 HP)

EACH TURN
  • A room has 4 cards - you must play exactly 3
  • The 4th card stays for the next room
  • You may skip a room (but not twice in a row)

COMBAT
  • Fight barehanded: take full monster damage
  • Use weapon: take (monster - weapon) damage
  • Weapon dulling: After killing a monster, weapon
    can only hit monsters with LOWER value (not equal)

POTIONS
  • Only ONE potion per turn (second is wasted)
  • Cannot heal above 20 HP

CONTROLS
  Tab/Arrows    Navigate cards
  Enter/Space   Play selected card
  S             Skip room
  L             View log
  ?             This help
  Q             Quit

Press any key to close"#;

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title("Help")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(help, area);
}

fn render_log_modal(f: &mut Frame, game: &GameState) {
    let area = centered_rect(70, 80, f.area());
    f.render_widget(Clear, area);

    let log_entries: Vec<Line> = game
        .log
        .iter()
        .rev()
        .take(20)
        .rev()
        .map(|s| Line::from(s.as_str()))
        .collect();

    let mut lines = vec![Line::from(Span::styled(
        "📜 ADVENTURE LOG",
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    lines.push(Line::from(""));
    lines.extend(log_entries);
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press any key to close",
        Style::default().fg(Color::DarkGray),
    )));

    let log = Paragraph::new(Text::from(lines)).block(
        Block::default()
            .title("Log")
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(Color::Blue)),
    );

    f.render_widget(log, area);
}

fn render_gameover_modal(f: &mut Frame, game: &GameState) {
    let area = centered_rect(50, 30, f.area());
    f.render_widget(Clear, area);

    let (title, color) = if game.won {
        ("🎉 VICTORY! 🎉", Color::Green)
    } else {
        ("💀 DEFEAT 💀", Color::Red)
    };

    let message = if game.won {
        "You conquered the dungeon!"
    } else {
        "The dungeon has claimed another soul..."
    };

    let lines = vec![
        Line::from(Span::styled(title, Style::default().add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(message),
        Line::from(""),
        Line::from(format!("Final Score: {}", game.calculate_score())),
        Line::from(""),
        Line::from("Play again? [Y/n]"),
    ];

    let gameover = Paragraph::new(Text::from(lines))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(color)),
        );

    f.render_widget(gameover, area);
}

fn render_quit_modal(f: &mut Frame) {
    let area = centered_rect(40, 25, f.area());
    f.render_widget(Clear, area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Quit game?",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Your progress will be lost."),
        Line::from(""),
        Line::from("[Y] Yes, quit"),
        Line::from("[N] No, keep playing"),
    ];

    let quit_modal = Paragraph::new(Text::from(lines))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .title(" Confirm ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::Yellow)),
        );

    f.render_widget(quit_modal, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
