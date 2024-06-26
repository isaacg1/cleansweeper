//#![feature(result_option_inspect)]
//#![feature(lint_reasons)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
//    reason = "Casting used freely, too much clutter otherwise"
)]
#![warn(clippy::nursery)]
#![allow(clippy::suboptimal_flops)]//, reason = "Less readable")]
#![warn(
    clippy::panic,
    clippy::shadow_unrelated,
    clippy::todo,
    clippy::unimplemented
)]

use druid::piet::{FontFamily, Text, TextLayout, TextLayoutBuilder};
use druid::widget::prelude::*;
use druid::widget::{Flex, Label};
use druid::{AppLauncher, Color, Data, MouseButton, Point, Rect, Size, WidgetExt, WindowDesc};

use std::ops::{Index, IndexMut};
use std::sync::Arc;

use clap::Parser;
use rand::prelude::*;

const NUM_FONT_SIZE: f64 = 36.0;
const SHRINK_CELL_SIZE: f64 = 40.0;
const SPACING: f64 = 5.0;
const MAX_ASPECT: f64 = 1.15;
const TORUS_OVERLAP: usize = 3;

const PINK: Color = Color::rgb8(0xff, 0xb7, 0xc5);
const BACKGROUND: Color = Color::grey8(23);

#[derive(Clone, Copy, PartialEq, Debug)]
enum CellState {
    SecretSafe,
    SecretBomb,
    Flagged,
    Opened,
    ExplodedSafe,
    ExplodedBomb,
}

#[derive(Clone, Data, PartialEq)]
struct Grid {
    storage: Arc<Vec<CellState>>,
    height: usize,
    width: usize,
    fraction: f64,
    torus: bool,
}
#[derive(Clone, Copy)]
struct GridPos {
    row: usize,
    col: usize,
}

impl Index<GridPos> for Grid {
    type Output = CellState;
    fn index(&self, pos: GridPos) -> &Self::Output {
        let idx = pos.row * self.width + pos.col;
        &self.storage[idx]
    }
}

impl IndexMut<GridPos> for Grid {
    fn index_mut(&mut self, pos: GridPos) -> &mut Self::Output {
        let idx = pos.row * self.width + pos.col;
        // Arc is just for cheaper comparisons
        Arc::make_mut(&mut self.storage).index_mut(idx)
    }
}

impl Grid {
    fn new(height: usize, width: usize, fraction: f64, torus: bool) -> Self {
        let mut grid = Self {
            storage: Arc::new(vec![CellState::ExplodedSafe; height * width]),
            height,
            width,
            fraction,
            torus,
        };
        grid.start();
        grid
    }
    fn iter_pos(&self) -> impl Iterator<Item = GridPos> + '_ {
        (0..self.height).flat_map(|row| (0..self.width).map(move |col| GridPos { row, col }))
    }
    fn neighbors(&self, pos: GridPos) -> [Option<GridPos>; 8] {
        let above = self.above(pos);
        let below = self.below(pos);
        let left = self.left(pos);
        let right = self.right(pos);
        let above_left = above.and_then(|apos| self.left(apos));
        let above_right = above.and_then(|apos| self.right(apos));
        let below_left = below.and_then(|bpos| self.left(bpos));
        let below_right = below.and_then(|bpos| self.right(bpos));
        [
            above,
            below,
            left,
            right,
            above_left,
            above_right,
            below_left,
            below_right,
        ]
    }
    // Number of neighboring unflagged bombs
    fn n_bombs(&self, pos: GridPos) -> usize {
        self.neighbors(pos)
            .iter()
            .filter(|m_neigh| {
                m_neigh.map_or(false, |neighbor| {
                    matches!(
                        self[neighbor],
                        CellState::SecretBomb | CellState::ExplodedBomb
                    )
                })
            })
            .count()
    }
    // Flood open. If cell is opened, and has n_bombs = 0, open all of its SecretSafe neighbors.
    fn flood(&mut self, pos: GridPos) {
        let mut to_flood = match self[pos] {
            CellState::Opened => vec![pos],
            CellState::Flagged => self
                .neighbors(pos)
                .iter()
                .filter_map(|p| *p)
                .filter(|p| self[*p] == CellState::Opened)
                .collect(),
            _ => unreachable!(),
        };
        while let Some(center) = to_flood.pop() {
            assert_eq!(self[center], CellState::Opened);
            if self.n_bombs(center) == 0 {
                for neighbor in self.neighbors(center).into_iter().flatten() {
                    match self[neighbor] {
                        CellState::SecretSafe => {
                            self[neighbor] = CellState::Opened;
                            to_flood.push(neighbor);
                        }
                        CellState::Opened | CellState::Flagged => (),
                        CellState::ExplodedSafe
                        | CellState::ExplodedBomb
                        | CellState::SecretBomb => unreachable!(),
                    }
                }
            }
        }
    }
    fn is_win(&self) -> bool {
        self.iter_pos()
            .all(|pos| matches!(self[pos], CellState::Opened | CellState::Flagged))
    }
    // Flag, return if exploded
    fn flag(&mut self, pos: GridPos) -> bool {
        match self[pos] {
            CellState::SecretBomb => {
                self[pos] = CellState::Flagged;
                self.flood(pos);
                false
            }
            CellState::SecretSafe => {
                self[pos] = CellState::ExplodedSafe;
                true
            }
            _ => false,
        }
    }
    // Open, return if exploded
    fn open(&mut self, pos: GridPos) -> bool {
        match self[pos] {
            CellState::SecretBomb => {
                self[pos] = CellState::ExplodedBomb;
                true
            }
            CellState::SecretSafe => {
                self[pos] = CellState::Opened;
                self.flood(pos);
                false
            }
            _ => false,
        }
    }
    // Start/restart. Randomize bombs, pick random 0 and open it.
    fn start(&mut self) {
        // Allow seeding?
        let mut rng = thread_rng();
        for row in 0..self.height {
            for col in 0..self.width {
                let pos = GridPos { row, col };
                let cell_state = if rng.gen::<f64>() < self.fraction {
                    CellState::SecretBomb
                } else {
                    CellState::SecretSafe
                };
                self[pos] = cell_state;
            }
        }
        let zero_positions: Vec<GridPos> = self
            .iter_pos()
            .filter(|&pos| self[pos] == CellState::SecretSafe && self.n_bombs(pos) == 0)
            .collect();
        // Zero_positions could be empty, so we have a fallback
        if !zero_positions.is_empty() {
            let index = rng.gen_range(0..zero_positions.len());
            let pos = zero_positions[index];
            let exploded = self.open(pos);
            assert!(!exploded);
        } else {
            let pos = GridPos {
                row: rng.gen_range(0..self.height),
                col: rng.gen_range(0..self.width),
            };
            self[pos] = CellState::SecretSafe;
            for neighbor in self.neighbors(pos) {
                if let Some(n_pos) = neighbor {
                    self[n_pos] = CellState::SecretSafe;
                }
            }
            let exploded = self.open(pos);
            assert!(!exploded);
        }
    }
    // Turn any explosions back into secret
    fn clear_explosions(&mut self) {
        for row in 0..self.height {
            for col in 0..self.width {
                let pos = GridPos { row, col };
                let new_state = match self[pos] {
                    CellState::ExplodedBomb => CellState::SecretBomb,
                    CellState::ExplodedSafe => CellState::SecretSafe,
                    _ => continue,
                };
                self[pos] = new_state;
            }
        }
    }
    fn above(&self, pos: GridPos) -> Option<GridPos> {
        if self.torus {
            let row = (pos.row + self.height - 1) % self.height;
            Some(GridPos { row, col: pos.col })
        } else {
            pos.row
                .checked_sub(1)
                .map(|row| GridPos { row, col: pos.col })
        }
    }
    fn below(&self, pos: GridPos) -> Option<GridPos> {
        if self.torus {
            let row = (pos.row + 1) % self.height;
            Some(GridPos { row, col: pos.col })
        } else {
            (pos.row < self.height - 1).then_some(GridPos {
                row: pos.row + 1,
                col: pos.col,
            })
        }
    }
    fn left(&self, pos: GridPos) -> Option<GridPos> {
        if self.torus {
            let col = (pos.col + self.width - 1) % self.width;
            Some(GridPos { row: pos.row, col })
        } else {
            pos.col
                .checked_sub(1)
                .map(|col| GridPos { row: pos.row, col })
        }
    }
    fn right(&self, pos: GridPos) -> Option<GridPos> {
        if self.torus {
            let col = (pos.col + 1) % self.width;
            Some(GridPos { row: pos.row, col })
        } else {
            (pos.col < self.width - 1).then_some(GridPos {
                row: pos.row,
                col: pos.col + 1,
            })
        }
    }
}

#[derive(Clone, Copy, PartialEq, Data)]
enum GameOver {
    Loss,
    Win,
    Ongoing,
}
#[derive(Clone, Data)]
struct AppData {
    grid: Grid,
    game_over: GameOver,
    easy_mode: bool,
}

struct CleansweeperWidget {
    cell_size: Size,
}
impl CleansweeperWidget {
    fn grid_pos(
        &self,
        p: Point,
        grid_height: usize,
        grid_width: usize,
        is_torus: bool,
    ) -> Option<GridPos> {
        let w0 = self.cell_size.width;
        let h0 = self.cell_size.height;
        if p.x < 0.0 || p.y < 0.0 || w0 == 0.0 || h0 == 0.0 {
            return None;
        }
        let row = (p.y / h0) as usize;
        let col = (p.x / w0) as usize;
        if row >= grid_height || col >= grid_width {
            if is_torus {
                if row < grid_height + TORUS_OVERLAP && col < grid_width + TORUS_OVERLAP {
                    Some(GridPos {
                        row: row % grid_height,
                        col: col % grid_width,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            Some(GridPos { row, col })
        }
    }
}

impl Widget<AppData> for CleansweeperWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut AppData, _env: &Env) {
        match event {
            Event::WindowConnected => ctx.request_paint(),
            Event::MouseDown(e) => {
                if data.game_over == GameOver::Ongoing {
                    match e.button {
                        MouseButton::Left => {
                            let grid_pos_opt = self.grid_pos(
                                e.pos,
                                data.grid.height,
                                data.grid.width,
                                data.grid.torus,
                            );
                            grid_pos_opt.inspect(|pos| {
                                let exploded = data.grid.flag(*pos);
                                if exploded {
                                    data.game_over = GameOver::Loss;
                                }
                            });
                        }
                        MouseButton::Right => {
                            let grid_pos_opt = self.grid_pos(
                                e.pos,
                                data.grid.height,
                                data.grid.width,
                                data.grid.torus,
                            );
                            grid_pos_opt.inspect(|pos| {
                                let exploded = data.grid.open(*pos);
                                if exploded {
                                    data.game_over = GameOver::Loss;
                                }
                            });
                        }
                        _ => (),
                    }
                    if data.grid.is_win() {
                        data.game_over = GameOver::Win;
                    }
                }
            }
            _ => {}
        }
    }
    fn lifecycle(
        &mut self,
        _ctx: &mut LifeCycleCtx,
        _event: &LifeCycle,
        _data: &AppData,
        _env: &Env,
    ) {
    }
    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &AppData, data: &AppData, _env: &Env) {
        if data.grid != old_data.grid {
            ctx.request_paint();
        }
    }
    fn layout(
        &mut self,
        _layout_ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &AppData,
        _env: &Env,
    ) -> Size {
        let Size {
            height: max_height,
            width: max_width,
        } = bc.max();
        let ideal_ratio = data.grid.height as f64 / data.grid.width as f64;
        let height_cap = max_width * ideal_ratio * MAX_ASPECT;
        let width_cap = (max_height / ideal_ratio) * MAX_ASPECT;
        Size {
            height: max_height.min(height_cap),
            width: max_width.min(width_cap),
        }
    }
    fn paint(&mut self, ctx: &mut PaintCtx, data: &AppData, _env: &Env) {
        let size: Size = ctx.size();
        let visual_width = data.grid.width + if data.grid.torus { TORUS_OVERLAP } else { 0 };
        let visual_height = data.grid.height + if data.grid.torus { TORUS_OVERLAP } else { 0 };
        let w0 = size.width / visual_width as f64;
        let h0 = size.height / visual_height as f64;
        let cell_size = Size {
            width: w0,
            height: h0,
        };
        self.cell_size = cell_size;
        let draw_size = Size {
            width: w0 - 2.0,
            height: h0 - 2.0,
        };
        let font_scale_down = ((w0.min(h0)) / SHRINK_CELL_SIZE).min(1.0);
        let font_size = NUM_FONT_SIZE * font_scale_down;
        for visual_row in 0..visual_height {
            for visual_col in 0..visual_width {
                let pos = GridPos {
                    row: visual_row % data.grid.height,
                    col: visual_col % data.grid.width,
                };
                let cell_state = data.grid[pos];
                let point = Point {
                    x: w0 * visual_col as f64 + 1.0,
                    y: h0 * visual_row as f64 + 1.0,
                };
                // Unknown is dark grey fill
                // Flagged is pink fill
                // Opened is white fill
                // Exploded is red fill
                // Number of unflagged neighbors written on top of white fill,
                // in varying colors. If none, no number.
                let rect = Rect::from_origin_size(point, draw_size);
                let fill_color = match cell_state {
                    CellState::SecretSafe | CellState::SecretBomb => Color::GRAY,
                    CellState::Flagged => PINK,
                    CellState::Opened => Color::WHITE,
                    CellState::ExplodedSafe | CellState::ExplodedBomb => Color::RED,
                };
                ctx.fill(rect, &fill_color);
                if cell_state == CellState::Opened {
                    let n_bombs = data.grid.n_bombs(pos);
                    if n_bombs > 0 {
                        let text_color = match n_bombs {
                            1 => Color::BLUE,
                            2 => Color::GREEN,
                            3 => Color::MAROON,
                            4 => Color::BLACK,
                            5 => Color::PURPLE,
                            6 => Color::AQUA,
                            7 => Color::OLIVE,
                            8 => Color::LIME,
                            _ => unreachable!(),
                        };
                        let text_layout = ctx
                            .text()
                            .new_text_layout(format!("{n_bombs}"))
                            .font(FontFamily::MONOSPACE, font_size)
                            .text_color(text_color)
                            .build()
                            .expect("Text failed");
                        let text_size = text_layout.size();
                        let new_corner = Point {
                            x: point.x + (w0 - text_size.width) / 2.0,
                            y: point.y + (h0 - text_size.height) / 2.0,
                        };
                        ctx.draw_text(&text_layout, new_corner);
                    }
                }
            }
        }
    }
}

fn make_widget() -> impl Widget<AppData> {
    let cleansweeper = CleansweeperWidget {
        cell_size: Size {
            width: 0.0,
            height: 0.0,
        },
    };
    let restart_button = Label::new("Restart")
        .with_text_size(NUM_FONT_SIZE)
        .on_click(move |_ctx, data: &mut AppData, _env| {
            data.game_over = GameOver::Ongoing;
            data.grid.start();
        })
        .center()
        .expand_width();
    let game_over_text = Label::new(|data: &AppData, _env: &_| match data.game_over {
        GameOver::Loss => {
            if data.easy_mode {
                "Undo?"
            } else {
                "Try again?"
            }
        }
        GameOver::Win => "You win!",
        GameOver::Ongoing => "Good luck!",
    })
    .with_text_size(NUM_FONT_SIZE)
    .on_click(move |_ctx, data: &mut AppData, _env| {
        if data.easy_mode {
            data.game_over = GameOver::Ongoing;
            data.grid.clear_explosions();
        }
    })
    .center()
    .expand_width();
    let bottom_row = Flex::row()
        .with_flex_child(restart_button, 1.0)
        .with_spacer(SPACING)
        .with_flex_child(game_over_text, 1.0);
    Flex::column()
        .with_flex_child(cleansweeper, 1.0)
        .with_spacer(SPACING)
        .with_child(bottom_row)
        .with_spacer(SPACING)
        .background(BACKGROUND)
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Height of Cleansweeper grid - default 16
    #[arg(short = 'H', long)]
    height: Option<usize>,

    /// Width of Cleansweeper grid - default 16
    #[arg(short, long)]
    width: Option<usize>,

    /// Fraction of cells which contain bombs - default 0.25
    #[arg(short, long)]
    fraction: Option<f64>,

    /// Easy mode - allows undos
    #[arg(short, long)]
    easy: bool,

    /// Torus mode - top and bottom, left and right connected
    #[arg(short, long)]
    torus: bool,
}

/*
TODO:
- Change board characteristics on restart
*/
fn main() {
    let args = Args::parse();
    let height = args.height.unwrap_or(16);
    let width = args.width.unwrap_or(16);
    let fraction = args.fraction.unwrap_or(0.25);
    assert!(fraction <= 1.0);
    assert!(fraction >= 0.0);
    let window = WindowDesc::new(make_widget())
        .window_size(Size {
            width: 800.,
            height: 800.,
        })
        .title("Cleansweeper");
    let mut grid = Grid::new(height, width, fraction, args.torus);
    grid.start();

    AppLauncher::with_window(window)
        .log_to_console()
        .launch(AppData {
            grid,
            game_over: GameOver::Ongoing,
            easy_mode: args.easy,
        })
        .expect("launch failed");
}
