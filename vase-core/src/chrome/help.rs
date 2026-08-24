//! The shortcut sheet: every binding, its miniature of the layout it acts on, and where both sit on
//! a centered card.

use crate::geometry::Rect;
use crate::input::{keymap, InputCommand, Key, KeyCode, Mods};

/// Padding between the card's edge and its content.
const PAD: f64 = 16.0;
const COL_W: f64 = 356.0;
const COL_GAP: f64 = 24.0;
const ROW_H: f64 = 19.0;
/// A section's own title row, including the air above it.
const SECTION_H: f64 = 30.0;
const HEADER_H: f64 = 28.0;
/// Width of the miniature at the head of a row, and the air after it.
const ART_W: f64 = 22.0;
const ART_H: f64 = 13.0;
const ART_GAP: f64 = 6.0;
/// Width of the chord column: every chord starts at its left edge, so the widest one sets this.
const KEYS_W: f64 = 108.0;
const KEYS_GAP: f64 = 10.0;

/// A cell of a miniature layout, in a 0..1 box with `y` running top to bottom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub rect: Rect,
    pub kind: CellKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    /// A pane that stays as it is.
    Plain,
    /// What the command acts on.
    Active,
    /// A pane that is empty, or the place one just left: outlined, not filled.
    Ghost,
}

const fn cell(x: f64, y: f64, w: f64, h: f64, kind: CellKind) -> Cell {
    Cell { rect: Rect { x, y, w, h }, kind }
}

use CellKind::{Active, Ghost, Plain};

/// Splitting leaves the new pane empty, which is what the picker then fills.
const SPLIT_RIGHT: &[Cell] = &[cell(0.0, 0.0, 0.48, 1.0, Plain), cell(0.52, 0.0, 0.48, 1.0, Ghost)];
const SPLIT_DOWN: &[Cell] = &[cell(0.0, 0.0, 1.0, 0.44, Plain), cell(0.0, 0.56, 1.0, 0.44, Ghost)];
const FOCUS: &[Cell] = &[cell(0.0, 0.0, 0.48, 1.0, Plain), cell(0.52, 0.0, 0.48, 1.0, Active)];
/// The divider has moved, so the focused pane is the wider one.
const RESIZE: &[Cell] = &[cell(0.0, 0.0, 0.28, 1.0, Plain), cell(0.32, 0.0, 0.68, 1.0, Active)];
/// The pane travelled: it is over there now, and where it was is empty.
const MOVE_PANE: &[Cell] = &[cell(0.0, 0.0, 0.48, 1.0, Ghost), cell(0.52, 0.0, 0.48, 1.0, Active)];
const ZOOM: &[Cell] = &[cell(0.0, 0.0, 1.0, 1.0, Active)];
/// Out of its split and into a tab of its own, over the pane it left.
const BREAK: &[Cell] = &[cell(0.0, 0.0, 0.48, 1.0, Ghost), cell(0.24, 0.16, 0.76, 0.84, Active)];
/// Two windows sharing one slot, the selected one in front.
const STACK: &[Cell] = &[cell(0.16, 0.0, 0.84, 0.84, Ghost), cell(0.0, 0.16, 0.84, 0.84, Active)];
/// A tab bar over a body: three tabs with the current one lit.
const TABS: &[Cell] = &[cell(0.0, 0.0, 0.3, 0.22, Plain), cell(0.35, 0.0, 0.3, 0.22, Active), cell(0.7, 0.0, 0.3, 0.22, Plain), cell(0.0, 0.3, 1.0, 0.7, Plain)];
/// A new tab is an empty one, appended at the end.
const NEW_TAB: &[Cell] = &[cell(0.0, 0.0, 0.3, 0.22, Plain), cell(0.35, 0.0, 0.3, 0.22, Plain), cell(0.7, 0.0, 0.3, 0.22, Active), cell(0.0, 0.3, 1.0, 0.7, Ghost)];
/// The tab has changed places with its neighbour.
const MOVE_TAB: &[Cell] = &[cell(0.0, 0.0, 0.3, 0.22, Ghost), cell(0.35, 0.0, 0.3, 0.22, Active), cell(0.7, 0.0, 0.3, 0.22, Plain), cell(0.0, 0.3, 1.0, 0.7, Plain)];
/// Two screens, the tab now on the far one.
const MONITOR: &[Cell] = &[cell(0.0, 0.0, 0.44, 0.22, Ghost), cell(0.0, 0.3, 0.44, 0.7, Plain), cell(0.56, 0.0, 0.44, 0.22, Active), cell(0.56, 0.3, 0.44, 0.7, Plain)];

/// One line of the sheet.
pub struct Row {
    /// The chord as shown, prefix included.
    pub keys: String,
    pub label: &'static str,
    pub art: Option<&'static [Cell]>,
    /// The binding this row stands for, so a test can hold the sheet to the keymap. `nested` picks
    /// which prefix's bindings it belongs to.
    pub binding: Option<(Key, InputCommand, bool)>,
}

pub struct Section {
    pub title: &'static str,
    pub rows: Vec<Row>,
}

fn shift(c: char) -> Key {
    Key { code: KeyCode::Char(c), mods: Mods { shift: true, ..Mods::default() } }
}

/// A row for a binding under the given prefix. `also` names the sibling chords the same command
/// answers to, which the sheet shows but does not check.
fn row(prefix: Key, key: Key, cmd: InputCommand, also: &str, label: &'static str, art: Option<&'static [Cell]>) -> Row {
    let nested = prefix == keymap::stack_prefix();
    Row { keys: format!("{} {}{also}", prefix.chord(), key.chord()), label, art, binding: Some((key, cmd, nested)) }
}

/// A row for keys that are not prefix bindings: what a picker or the command line answers to.
fn note(keys: &str, label: &'static str) -> Row {
    Row { keys: keys.to_string(), label, art: None, binding: None }
}

/// Every shortcut, grouped as the docs group them.
pub fn sections() -> Vec<Section> {
    use InputCommand as I;
    let p = keymap::prefix();
    let s = keymap::stack_prefix();
    vec![
        Section {
            title: "Tabs",
            rows: vec![
                row(p, Key::ch('c'), I::NewTab, "", "new tab", Some(NEW_TAB)),
                row(p, Key::ch('.'), I::StackNext, " / ,", "next / previous tab", Some(TABS)),
                row(p, Key::ch('1'), I::SelectBarTab(1), "…9", "jump to tab n", Some(TABS)),
                row(p, Key::ch('t'), I::Rename, "", "rename the tab", None),
                row(p, shift('.'), I::MoveTabNext, " / ⇧,", "move the tab right / left", Some(MOVE_TAB)),
                row(p, shift(']'), I::MoveTabMonitorNext, " / ⇧[", "to the next / previous monitor", Some(MONITOR)),
                row(p, Key::ch('w'), I::WindowSwitcher, "", "window switcher", None),
                row(p, Key::ch('l'), I::LastTab, "", "jump to the last window", None),
            ],
        },
        Section {
            title: "Panes",
            rows: vec![
                row(p, Key::ch('\\'), I::SplitH, "", "split right", Some(SPLIT_RIGHT)),
                row(p, Key::ch('-'), I::SplitV, "", "split down", Some(SPLIT_DOWN)),
                row(p, Key::plain(KeyCode::Right), I::FocusRight, " ← ↑ ↓", "move focus", Some(FOCUS)),
                row(p, Key { code: KeyCode::Right, mods: Mods { shift: true, ..Mods::default() } }, I::ResizeRight, " / ⇧HJKL", "resize", Some(RESIZE)),
                row(p, Key::meta(KeyCode::Right), I::MoveRight, " / ⌘HJKL", "move the pane", Some(MOVE_PANE)),
                row(p, Key::ch('z'), I::ZoomToggle, "", "zoom the pane", Some(ZOOM)),
                row(p, Key::ch('x'), I::BreakPane, "", "break it into its own tab", Some(BREAK)),
                row(p, Key::ch('m'), I::WarpCursor, "", "move the cursor to the pane", None),
                row(p, Key { code: KeyCode::Char('r'), mods: Mods { ctrl: true, ..Mods::default() } }, I::Resync, "", "re-place every window", None),
            ],
        },
        Section {
            title: "Stacks",
            rows: vec![
                row(p, Key::ch('s'), I::Stackify, "", "stack the pane / add to it", Some(STACK)),
                row(p, Key::ch(']'), I::StackFocusNext, " / [", "cycle the stack", Some(STACK)),
                row(s, Key::ch('.'), I::StackFocusNext, " / ⌥e ,", "cycle the stack", None),
                row(s, Key::ch('1'), I::StackSelectItem(1), "…9", "select stack item n", None),
                row(s, Key::ch('t'), I::StackRename, "", "rename the stack item", None),
                row(s, shift('.'), I::StackMoveNext, " / ⌥e ⇧,", "reorder the stack item", None),
            ],
        },
        Section {
            title: "Prompt and pickers",
            rows: vec![
                row(p, shift(';'), I::CommandLine, "", "command line", None),
                note("", ":q  :rename  :close  :split  :vsplit  :zoom  :tab n"),
                note("type", "search the open picker"),
                note("j / k / 1…9", "choose a row"),
                note("⏎ / esc", "open it / cancel"),
                row(p, shift('/'), I::Help, "", "this sheet", None),
                row(p, Key::ch('q'), I::Quit, "", "quit, restoring every window", None),
            ],
        },
    ]
}

/// How a painter styles one piece of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextStyle {
    /// The card's own heading.
    Title,
    /// A section heading.
    Section,
    /// A chord, starting at its column's left edge so every chord lines up.
    Keys,
    /// What the chord does.
    Label,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    /// Card-local, top-left origin.
    pub rect: Rect,
    pub text: String,
    pub style: TextStyle,
}

/// A laid-out sheet: the card, and everything on it already placed.
#[derive(Debug, Clone, PartialEq)]
pub struct HelpLayout {
    /// The card's rect, centered on the screen it was laid out for.
    pub rect: Rect,
    pub texts: Vec<Text>,
    /// The miniatures' cells, scaled into place.
    pub cells: Vec<Cell>,
}

/// Lay the sheet out as a card centered on `screen`.
pub fn layout(screen: Rect) -> HelpLayout {
    let sections = sections();
    // Two columns, split where the rows balance, so the card is as square as its content allows.
    let heights: Vec<f64> = sections.iter().map(|s| SECTION_H + s.rows.len() as f64 * ROW_H).collect();
    let total: f64 = heights.iter().sum();
    let mut split = 1;
    let mut run = 0.0;
    for (i, h) in heights.iter().enumerate() {
        if run + h / 2.0 > total / 2.0 {
            split = i.max(1);
            break;
        }
        run += h;
        split = i + 1;
    }
    let col_h = |range: std::ops::Range<usize>| heights[range].iter().sum::<f64>();
    let body_h = col_h(0..split).max(col_h(split..sections.len()));

    let w = 2.0 * PAD + 2.0 * COL_W + COL_GAP;
    let h = 2.0 * PAD + HEADER_H + body_h;
    // Centered, but never off the left or top edge of a screen too small to hold the whole sheet.
    let rect = Rect::new(screen.x + ((screen.w - w) / 2.0).max(0.0), screen.y + ((screen.h - h) / 2.0).max(0.0), w, h);

    let mut texts = vec![Text {
        rect: Rect::new(PAD, PAD, w - 2.0 * PAD, HEADER_H),
        text: format!("vase shortcuts   ·   {} arms the prefix   ·   press any key to close", keymap::prefix().chord()),
        style: TextStyle::Title,
    }];
    let mut cells = Vec::new();
    let mut y = [PAD + HEADER_H, PAD + HEADER_H];
    for (i, section) in sections.iter().enumerate() {
        let col = usize::from(i >= split);
        let x = PAD + col as f64 * (COL_W + COL_GAP);
        texts.push(Text { rect: Rect::new(x, y[col], COL_W, SECTION_H), text: section.title.to_string(), style: TextStyle::Section });
        y[col] += SECTION_H;
        for row in &section.rows {
            // Chord, then its miniature, then what it does: the diagram sits with the action it shows.
            let art_x = x + KEYS_W + KEYS_GAP;
            texts.push(Text { rect: Rect::new(x, y[col], KEYS_W, ROW_H), text: row.keys.clone(), style: TextStyle::Keys });
            if let Some(art) = row.art {
                let box_ = Rect::new(art_x, y[col] + (ROW_H - ART_H) / 2.0, ART_W, ART_H);
                cells.extend(art.iter().map(|c| Cell { rect: scale(c.rect, box_), kind: c.kind }));
            }
            // A note carries no chord, so it runs the whole column.
            let label_x = if row.keys.is_empty() { x } else { art_x + ART_W + ART_GAP };
            texts.push(Text { rect: Rect::new(label_x, y[col], COL_W - (label_x - x), ROW_H), text: row.label.to_string(), style: TextStyle::Label });
            y[col] += ROW_H;
        }
    }
    HelpLayout { rect, texts, cells }
}

/// A unit-box rect scaled into `into`.
fn scale(unit: Rect, into: Rect) -> Rect {
    Rect::new(into.x + unit.x * into.w, into.y + unit.y * into.h, unit.w * into.w, unit.h * into.h)
}
