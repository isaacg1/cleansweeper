#![feature(result_option_inspect)]

use druid::piet::{FontFamily, Text, TextLayout, TextLayoutBuilder};
use druid::widget::prelude::*;
use druid::widget::{Flex, Label};
use druid::{AppLauncher, Color, Data, MouseButton, Point, Rect, Size, WidgetExt, WindowDesc};
use std::ops::{Index, IndexMut};
use std::sync::Arc;

use rand::prelude::*;

const GRID_HEIGHT: usize = 16;
const GRID_WIDTH: usize = 30;
const POOL_SIZE: usize = GRID_HEIGHT * GRID_WIDTH;
const BOMB_PROB: f64 = 0.25;
const NUM_FONT_SIZE: f64 = 36.0;
const VERT_SPACING: f64 = 5.0;

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
}
#[derive(Clone, Copy)]
struct GridPos {
    row: usize,
    col: usize,
}

impl Index<GridPos> for Grid {
    type Output = CellState;
    fn index(&self, pos: GridPos) -> &Self::Output {
        let idx = pos.row * GRID_WIDTH + pos.col;
        &self.storage[idx]
    }
}

impl IndexMut<GridPos> for Grid {
    fn index_mut(&mut self, pos: GridPos) -> &mut Self::Output {
        let idx = pos.row * GRID_WIDTH + pos.col;
        Arc::make_mut(&mut self.storage).index_mut(idx)
    }
}

impl Grid {
    fn new() -> Grid {
        let mut grid = Grid {
            storage: Arc::new(vec![CellState::ExplodedSafe; POOL_SIZE]),
        };
        grid.start();
        grid
    }
    fn neighbors(pos: GridPos) -> [Option<GridPos>; 8] {
        let above = pos.above();
        let below = pos.below();
        let left = pos.left();
        let right = pos.right();
        let above_left = above.and_then(GridPos::left);
        let above_right = above.and_then(GridPos::right);
        let below_left = below.and_then(GridPos::left);
        let below_right = below.and_then(GridPos::right);
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
        Grid::neighbors(pos)
            .iter()
            .filter(|pos| {
                pos.map_or(false, |pos| {
                    matches!(self[pos], CellState::SecretBomb | CellState::ExplodedBomb)
                })
            })
            .count()
    }
    // Flood open. If cell is opened, and has n_bombs = 0, open all of its SecretSafe neighbors.
    fn flood(&mut self, pos: GridPos) {
        let mut to_flood = match self[pos] {
            CellState::Opened => vec![pos],
            CellState::Flagged => Grid::neighbors(pos)
                .iter()
                .filter_map(|p| *p)
                .filter(|p| self[*p] == CellState::Opened)
                .collect(),
            _ => unreachable!(),
        };
        while let Some(center) = to_flood.pop() {
            assert_eq!(self[center], CellState::Opened);
            if self.n_bombs(center) == 0 {
                for neighbor in Grid::neighbors(center).into_iter().flatten() {
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
        for row in 0..GRID_HEIGHT {
            for col in 0..GRID_WIDTH {
                let pos = GridPos { row, col };
                let cell_state = if rng.gen::<f64>() < BOMB_PROB {
                    CellState::SecretBomb
                } else {
                    CellState::SecretSafe
                };
                self[pos] = cell_state;
            }
        }
        let mut zero_positions = vec![];
        for row in 0..GRID_HEIGHT {
            for col in 0..GRID_WIDTH {
                let pos = GridPos { row, col };
                let n_bombs = self.n_bombs(pos);
                if self[pos] == CellState::SecretSafe && n_bombs == 0 {
                    zero_positions.push(pos);
                }
            }
        }
        // Zero_positions could be empty, but it's super rare, so I'd rather just crash.
        assert!(!zero_positions.is_empty());
        let index = rng.gen_range(0..zero_positions.len());
        let pos = zero_positions[index];
        let exploded = self.open(pos);
        assert!(!exploded);
    }
}

impl GridPos {
    fn above(self) -> Option<GridPos> {
        if self.row == 0 {
            None
        } else {
            Some(GridPos {
                row: self.row - 1,
                col: self.col,
            })
        }
    }
    fn below(self) -> Option<GridPos> {
        if self.row >= GRID_HEIGHT - 1 {
            None
        } else {
            Some(GridPos {
                row: self.row + 1,
                col: self.col,
            })
        }
    }
    fn left(self) -> Option<GridPos> {
        if self.col == 0 {
            None
        } else {
            Some(GridPos {
                row: self.row,
                col: self.col - 1,
            })
        }
    }
    fn right(self) -> Option<GridPos> {
        if self.col >= GRID_WIDTH - 1 {
            None
        } else {
            Some(GridPos {
                row: self.row,
                col: self.col + 1,
            })
        }
    }
}
#[derive(Clone, Data)]
struct AppData {
    grid: Grid,
    game_over: bool,
}

struct CleansweeperWidget {
    cell_size: Size,
}
impl CleansweeperWidget {
    fn grid_pos(&self, p: Point) -> Option<GridPos> {
        let w0 = self.cell_size.width;
        let h0 = self.cell_size.height;
        if p.x < 0.0 || p.y < 0.0 || w0 == 0.0 || h0 == 0.0 {
            return None;
        }
        let row = (p.y / h0) as usize;
        let col = (p.x / w0) as usize;
        if row >= GRID_HEIGHT || col >= GRID_WIDTH {
            return None;
        }
        Some(GridPos { row, col })
    }
}

impl Widget<AppData> for CleansweeperWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut AppData, _env: &Env) {
        match event {
            Event::WindowConnected => ctx.request_paint(),
            Event::MouseDown(e) => {
                if !data.game_over {
                    match e.button {
                        MouseButton::Left => {
                            let grid_pos_opt = self.grid_pos(e.pos);
                            grid_pos_opt.inspect(|pos| {
                                let exploded = data.grid.flag(*pos);
                                if exploded {
                                    data.game_over = true;
                                }
                            });
                        }
                        MouseButton::Right => {
                            let grid_pos_opt = self.grid_pos(e.pos);
                            grid_pos_opt.inspect(|pos| {
                                let exploded = data.grid.open(*pos);
                                if exploded {
                                    data.game_over = true;
                                }
                            });
                        }
                        _ => (),
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
        _data: &AppData,
        _env: &Env,
    ) -> Size {
        bc.max()
    }
    fn paint(&mut self, ctx: &mut PaintCtx, data: &AppData, _env: &Env) {
        let size: Size = ctx.size();
        let w0 = size.width / GRID_WIDTH as f64;
        let h0 = size.height / GRID_HEIGHT as f64;
        let cell_size = Size {
            width: w0,
            height: h0,
        };
        self.cell_size = cell_size;
        let draw_size = Size {
            width: w0 - 2.0,
            height: h0 - 2.0,
        };
        for row in 0..GRID_HEIGHT {
            for col in 0..GRID_WIDTH {
                let pos = GridPos { row, col };
                let cell_state = data.grid[pos];
                let point = Point {
                    x: w0 * col as f64 + 1.0,
                    y: h0 * row as f64 + 1.0,
                };
                // Unknown is dark grey fill
                // Flagged is pink fill
                // Opened is white fill
                // Exploded is red fill
                // Number of unflagged neighbors written on top of white fill,
                // in varying colors. If none, no number.
                let rect = Rect::from_origin_size(point, draw_size);
                let color = match cell_state {
                    CellState::SecretSafe | CellState::SecretBomb => Color::GRAY,
                    CellState::Flagged => Color::FUCHSIA,
                    CellState::Opened => Color::WHITE,
                    CellState::ExplodedSafe | CellState::ExplodedBomb => Color::RED,
                };
                ctx.fill(rect, &color);
                if cell_state == CellState::Opened {
                    let n_bombs = data.grid.n_bombs(pos);
                    if n_bombs > 0 {
                        let color = match n_bombs {
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
                        let text = ctx.text();
                        let text_layout = text
                            .new_text_layout(format!("{n_bombs}"))
                            .font(FontFamily::MONOSPACE, NUM_FONT_SIZE)
                            .text_color(color)
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
            data.game_over = false;
            data.grid.start();
        });
    Flex::column()
        .with_flex_child(cleansweeper, 1.0)
        .with_spacer(VERT_SPACING)
        .with_child(restart_button)
        .with_spacer(VERT_SPACING)
        .background(BACKGROUND)
}

/*
TODO:
- Make width and height separate
- Add "You win!/Try again?"
*/
fn main() {
    let window = WindowDesc::new(make_widget())
        .window_size(Size {
            width: 800.,
            height: 800.,
        })
        .title("Cleansweeper");
    let mut grid = Grid::new();
    grid.start();

    AppLauncher::with_window(window)
        .log_to_console()
        .launch(AppData {
            grid,
            game_over: false,
        })
        .expect("launch failed");
}
