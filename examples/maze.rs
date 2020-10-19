use moving::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use std::{
    collections::{HashSet, VecDeque},
    ops::Range,
    time::{Duration, Instant},
};

#[derive(Debug)]
struct Random {
    state: u64,
}

impl Random {
    fn new() -> Random {
        let instant = Instant::now();
        std::thread::yield_now();
        let state = instant.elapsed().as_nanos() as u64;
        Self { state }
    }

    fn u64(&mut self) -> u64 {
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        self.state.wrapping_mul(0x2545F4914F6CDD1) >> 33
    }

    fn range(&mut self, range: Range<usize>) -> usize {
        range.start + ((self.u64() as usize) % (range.end - range.start))
    }

    fn choose<T: Copy>(&mut self, array: &[T]) -> Option<T> {
        if array.is_empty() {
            return None;
        }
        Some(array[(self.u64() as usize) % array.len()])
    }
}

#[derive(Debug)]
struct Maze {
    cells: Vec<bool>,
    dimensions: (usize, usize),
}

impl Maze {
    fn new(width: usize, height: usize) -> Self {
        let mut maze = Self {
            cells: vec![true; width * height],
            dimensions: (width, height),
        };
        maze.regen();
        maze
    }

    fn regen(&mut self) {
        //let i = Instant::now();
        let mut random = Random::new();
        //for _ in 0..1000 {
        self.cells.iter_mut().for_each(|c| *c = true);
        #[derive(Debug, Copy, Clone)]
        enum Direction {
            Up,
            Right,
            Down,
            Left,
        }
        let mut visited_cells = vec![false; self.cells.len()];
        let mut cursor = (1, 1);
        let mut backtrack = Vec::new();
        loop {
            visited_cells[cursor.1 * self.cols() + cursor.0] = true;
            self.set_cell(cursor.0, cursor.1, false);
            let mut directions = Vec::with_capacity(4);
            if cursor.1 > 0
                && self.diggable_cell(cursor.0, cursor.1 - 1)
                && !visited_cells[(cursor.1 - 1) * self.cols() + cursor.0]
            {
                directions.push(Direction::Up);
            }
            if self.diggable_cell(cursor.0 + 1, cursor.1)
                && !visited_cells[cursor.1 * self.cols() + cursor.0 + 1]
            {
                directions.push(Direction::Right);
            }
            if self.diggable_cell(cursor.0, cursor.1 + 1)
                && !visited_cells[(cursor.1 + 1) * self.cols() + cursor.0]
            {
                directions.push(Direction::Down);
            }
            if cursor.0 > 0
                && self.diggable_cell(cursor.0 - 1, cursor.1)
                && !visited_cells[cursor.1 * self.cols() + (cursor.0 - 1)]
            {
                directions.push(Direction::Left);
            }
            if let Some(direction) = random.choose(&directions) {
                backtrack.push(cursor);
                match direction {
                    Direction::Up => {
                        cursor.1 -= 1;
                    }
                    Direction::Right => {
                        cursor.0 += 1;
                    }
                    Direction::Down => {
                        cursor.1 += 1;
                    }
                    Direction::Left => {
                        cursor.0 -= 1;
                    }
                }
            } else {
                if let Some(new_coords) = backtrack.pop() {
                    cursor = new_coords;
                } else {
                    break;
                }
            }
        }
        //}
        //println!("{:?}", i.elapsed() / 1000);
    }

    // Is a cell diggable?, for that it will not be the border and must not have less than three walls
    fn diggable_cell(&self, x: usize, y: usize) -> bool {
        if x == 0 || y == 0 || x >= self.cols() - 1 || y >= self.rows() - 1 {
            return false;
        }
        let mut walls = 0;
        walls += self.cell(x, y - 1) as usize;
        walls += self.cell(x + 1, y) as usize;
        walls += self.cell(x, y + 1) as usize;
        walls += self.cell(x - 1, y) as usize;

        let mut dwalls = 0;
        dwalls += self.cell(x + 1, y - 1) as usize;
        dwalls += self.cell(x + 1, y + 1) as usize;
        dwalls += self.cell(x - 1, y + 1) as usize;
        dwalls += self.cell(x - 1, y - 1) as usize;
        (walls >= 3) && (dwalls >= 2)
    }

    fn cell(&self, x: usize, y: usize) -> bool {
        self.cells[y * self.cols() + x]
    }

    fn set_cell(&mut self, x: usize, y: usize, value: bool) {
        let ofs = y * self.cols() + x;
        self.cells[ofs] = value;
    }

    fn cols(&self) -> usize {
        self.dimensions.0
    }

    fn rows(&self) -> usize {
        self.dimensions.1
    }

    fn cells(&self) -> impl Iterator<Item = &bool> {
        self.cells.iter()
    }

    /// Returns the shortest path from a point to another one(and panics if the path not exist, that case would not be possible in a maze)
    fn trace_path(&self, a: (usize, usize), b: (usize, usize)) -> Vec<(usize, usize)> {
        // Breadth-First Search algorithm
        let mut i = Instant::now();
        let mut previous_cells = vec![None; self.cells.len()];
        let mut queue = VecDeque::new();
        queue.push_front(a);
        let mut previous_cell = (0, 0); // The value is not important, can be anything
        while let Some((x, y)) = queue.pop_back() {
            previous_cells[y * self.cols() + x] = Some(previous_cell);
            for move_ in self
                .moves(x, y)
                .into_iter()
                .filter(|p| *p != a && previous_cells[p.1 * self.cols() + p.0].is_none())
            {
                if move_ == b {
                    let mut path = Vec::new();
                    let mut cursor = move_;
                    path.push(cursor);
                    path.push((x, y));
                    cursor = (x, y);
                    if cursor != a {
                        loop {
                            let previous_cell =
                                previous_cells[cursor.1 * self.cols() + cursor.0].unwrap();
                            path.push(previous_cell);
                            if previous_cell == a {
                                break;
                            }
                            cursor = previous_cell;
                        }
                    }
                    return path.into_iter().rev().collect();
                }
                queue.push_front(move_);
            }
            previous_cell = (x, y);
        }
        panic!(
            "There is not a path from (x: {}, y: {}) to (x: {}, y: {})",
            a.0, a.1, b.0, b.1
        );
    }

    fn moves(&self, x: usize, y: usize) -> Vec<(usize, usize)> {
        let mut result = Vec::with_capacity(4);
        if y >= 2 && !self.cell(x, y - 1) {
            result.push((x, y - 1));
        }
        if x < self.cols() - 2 && !self.cell(x + 1, y) {
            result.push((x + 1, y));
        }
        if y < self.rows() - 2 && !self.cell(x, y + 1) {
            result.push((x, y + 1));
        }
        if x >= 2 && !self.cell(x - 1, y) {
            result.push((x - 1, y));
        }
        result
    }
}

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Maze(Moving example)")
        .build(&event_loop)
        .unwrap();
    let start = Instant::now();
    let maze = Maze::new(800, 600);
    let mut last_frame = Instant::now();
    let fps = Duration::from_millis(1000 / 144);
    let mut last_move = Instant::now();
    let mut traveler_location = (1, 1);
    let mut random = Random::new();
    let mut traveling_path = Vec::new();
    let mut traveling_path_cells = HashSet::new();
    event_loop.run(move |event, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                println!("The close button was pressed; stopping");
                *control_flow = ControlFlow::Exit
            }
            Event::MainEventsCleared => {
                if last_frame.elapsed() >= fps {
                    last_frame = Instant::now();
                } else {
                    return;
                }
                let size = window.size();
                let w = size.width as usize;
                let h = size.height as usize;
                let frame_buffer = window.frame_buffer();
                //frame_buffer.iter_mut().for_each(|p| *p = 255);
                let maze_cell = 1;
                let maze_x = (w - (maze.cols() * maze_cell)) / 2;
                let maze_y = (h - (maze.rows() * maze_cell)) / 2;

                let maze_cols = maze.cols();

                let mut x = 0;
                let mut y = 0;
                for cell in maze.cells().copied() {
                    let px = maze_x + x * maze_cell;
                    let py = maze_y + y * maze_cell;
                    if px + 30 < w {
                        for y in py..py + maze_cell {
                            if y >= h {
                                break;
                            }
                            for x in px..px+maze_cell {
                                let color;
                                if cell {
                                    color = 0xff000000;
                                }
                                else if traveling_path_cells.contains(&(x, y)) {
                                    color = 0xff00ffff;
                                }
                                else {
                                    color = 0xffffffff;
                                }
                                surface.put_u32_pixel(x, y, color);
                            }
                        }
                    }
                    x += 1;
                    if x >= maze_cols {
                        x = 0;
                        y += 1;
                    }
                }
                if last_move.elapsed() >= Duration::from_secs(2) {
                    last_move = Instant::now();
                    let mut to_x = 0;
                    let mut to_y = 0;
                    // Work-around because of the maze bugs
                    loop {
                        to_x = random.range(2..maze.cols() - 1);
                        to_y = random.range(2..maze.rows() - 1);
                        if !maze.cell(to_x, to_y) {
                            break;
                        }
                        if let Some(new_pos) = maze.moves(to_x, to_y).first().copied() {
                            to_x = new_pos.0;
                            to_y = new_pos.1;
                        }
                    }
                    traveling_path = maze.trace_path(traveler_location, (to_x, to_y));
                    traveling_path_cells.clear();
                    traveling_path_cells.reserve(traveling_path.len());
                    traveling_path.iter().copied().for_each(|c| {
                        traveling_path_cells.insert(c);
                    });
                    traveler_location = (to_x, to_y);
                }
                window.redraw();
            }
            _ => (),
        }
    });
}
