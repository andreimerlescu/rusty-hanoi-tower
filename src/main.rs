use eframe::egui;
use rodio::{OutputStream, Sink};
use std::collections::VecDeque;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

// A peg is just its index into self.pegs
type Peg = usize;

#[derive(Clone)]
struct Move {
    from: Peg,
    to: Peg,
    disk: usize,
}

struct AnimatedDisk {
    disk: usize,
    from_peg: Peg,
    to_peg: Peg,
    progress: f32,
}

// ---------------------------------------------------------------------------
// Level
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Level {
    number: usize,    // 1–330
    num_pegs: usize,  // 3–33
    num_disks: usize, // grows with tier and variation
    variation: usize, // 1–10
}

impl Level {
    fn from_number(n: usize) -> Self {
        let n = n.clamp(1, 330);
        let variation = ((n - 1) % 10) + 1; // 1–10, cycles every 10 levels
        let tier = (n - 1) / 10;            // 0–32, increments every 10 levels
        let num_pegs = (3 + tier).min(33);
        let num_disks = 3 + tier + variation / 3; // 3 at tier 0 var 1, grows steadily
        Self { number: n, num_pegs, num_disks, variation }
    }

    fn label(&self) -> String {
        format!(
            "Level {} — {} pegs, {} disks (variation {}/10)",
            self.number, self.num_pegs, self.num_disks, self.variation
        )
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

struct HanoiApp {
    level: Level,
    pegs: Vec<VecDeque<usize>>,
    history: Vec<Move>,
    selected: Option<Peg>,
    animated_disk: Option<AnimatedDisk>,
    move_count: usize,
    auto_solving: bool,
    auto_step: usize,
    solution: Vec<(Peg, Peg)>,
    show_hint: bool,

    // Audio
    _stream: Option<OutputStream>,
    sink: Option<Arc<Sink>>,
}

impl Default for HanoiApp {
    fn default() -> Self {
        let (_stream, stream_handle) = OutputStream::try_default().ok().unzip();
        let sink = stream_handle.and_then(|h| Sink::try_new(&h).ok());

        let mut app = Self {
            level: Level::from_number(1),
            pegs: Vec::new(),
            history: Vec::new(),
            selected: None,
            animated_disk: None,
            move_count: 0,
            auto_solving: false,
            auto_step: 0,
            solution: Vec::new(),
            show_hint: false,
            _stream,
            sink: sink.map(Arc::new),
        };
        app.reset();
        app
    }
}

impl HanoiApp {
    fn reset(&mut self) {
        let num_pegs = self.level.num_pegs;
        let num_disks = self.level.num_disks;

        self.pegs = vec![VecDeque::new(); num_pegs];
        for i in (1..=num_disks).rev() {
            self.pegs[0].push_back(i);
        }

        self.history.clear();
        self.selected = None;
        self.animated_disk = None;
        self.move_count = 0;
        self.auto_solving = false;
        self.auto_step = 0;
        self.solution.clear();
        self.show_hint = false;
    }

    fn go_to_level(&mut self, n: usize) {
        self.level = Level::from_number(n);
        self.reset();
        self.play_sound("click");
    }

    fn is_solved(&self) -> bool {
        self.pegs
            .last()
            .map_or(false, |p| p.len() == self.level.num_disks)
    }

    /// Play a sound effect. Gracefully does nothing if audio is unavailable.
    fn play_sound(&self, name: &str) {
        if let Some(sink) = &self.sink {
            let path = format!("assets/{}.wav", name);
            if Path::new(&path).exists() {
                if let Ok(file) = File::open(&path) {
                    if let Ok(source) = rodio::Decoder::new(BufReader::new(file)) {
                        let sink_clone = sink.clone();
                        sink_clone.append(source);
                        sink_clone.play();
                    }
                }
            }
        }
    }

    fn is_valid_move(&self, from: Peg, to: Peg) -> bool {
        let disk = match self.pegs[from].back() {
            Some(&d) => d,
            None => return false,
        };
        match self.pegs[to].back() {
            Some(&top) => disk < top,
            None => true,
        }
    }

    fn make_move(&mut self, from: Peg, to: Peg) -> bool {
        if self.animated_disk.is_some() {
            return false;
        }
        if !self.is_valid_move(from, to) {
            self.play_sound("invalid");
            return false;
        }

        let disk = self.pegs[from].pop_back().unwrap();
        self.pegs[to].push_back(disk);
        self.history.push(Move { from, to, disk });
        self.move_count += 1;

        self.animated_disk = Some(AnimatedDisk {
            disk,
            from_peg: from,
            to_peg: to,
            progress: 0.0,
        });

        self.play_sound("move");

        if self.is_solved() {
            self.play_sound("win");
        }

        true
    }

    fn undo(&mut self) {
        if let Some(last) = self.history.pop() {
            let disk = self.pegs[last.to].pop_back().unwrap();
            self.pegs[last.from].push_back(disk);
            self.move_count = self.move_count.saturating_sub(1);
            self.animated_disk = None;
            self.play_sound("move");
        }
    }

    // -----------------------------------------------------------------------
    // Solvers
    // -----------------------------------------------------------------------

    fn generate_solution(&mut self) {
        self.solution.clear();
        let num_pegs = self.level.num_pegs;
        let num_disks = self.level.num_disks;
        let dst = num_pegs - 1;

        if num_pegs == 3 {
            hanoi_3peg(num_disks, 0, dst, 1, &mut self.solution);
        } else {
            let aux: Vec<Peg> = (1..dst).collect();
            frame_stewart(num_disks, 0, dst, aux, &mut self.solution);
        }
    }

    fn hint(&mut self) {
        if self.solution.is_empty() {
            self.generate_solution();
        }
        self.show_hint = true;
        self.play_sound("hint");
    }
}

// ---------------------------------------------------------------------------
// Solvers (free functions)
// ---------------------------------------------------------------------------

fn hanoi_3peg(n: usize, src: Peg, dst: Peg, aux: Peg, moves: &mut Vec<(Peg, Peg)>) {
    if n == 0 {
        return;
    }
    if n == 1 {
        moves.push((src, dst));
        return;
    }
    hanoi_3peg(n - 1, src, aux, dst, moves);
    moves.push((src, dst));
    hanoi_3peg(n - 1, aux, dst, src, moves);
}

fn frame_stewart(
    n: usize,
    src: Peg,
    dst: Peg,
    aux: Vec<Peg>,
    moves: &mut Vec<(Peg, Peg)>,
) {
    if n == 0 {
        return;
    }
    if n == 1 {
        moves.push((src, dst));
        return;
    }
    if aux.is_empty() {
        return;
    }
    if aux.len() == 1 {
        hanoi_3peg(n, src, dst, aux[0], moves);
        return;
    }

    // optimal k: move k disks aside using all pegs, then n-k using n-1 pegs, then k back
    let k = (n as f64 - (2.0 * n as f64).sqrt()).round().max(1.0) as usize;

    let spare = aux[0];
    let rest: Vec<Peg> = aux[1..].to_vec();

    let aux1: Vec<Peg> = [&[dst], rest.as_slice()].concat();
    frame_stewart(k, src, spare, aux1, moves);

    let aux2: Vec<Peg> = rest.clone();
    frame_stewart(n - k, src, dst, aux2, moves);

    let aux3: Vec<Peg> = [&[src], rest.as_slice()].concat();
    frame_stewart(k, spare, dst, aux3, moves);
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

impl eframe::App for HanoiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🗼 Tower of Hanoi");
            ui.label(self.level.label());
            ui.separator();

            // Controls
            ui.horizontal(|ui| {
                if ui.button("⬅ Prev Level").clicked() && self.level.number > 1 {
                    let n = self.level.number - 1;
                    self.go_to_level(n);
                }
                if ui.button("Next Level ➡").clicked() && self.level.number < 330 {
                    let n = self.level.number + 1;
                    self.go_to_level(n);
                }

                ui.separator();

                if ui.button("Reset").clicked() {
                    self.reset();
                    self.play_sound("click");
                }
                if ui.button("Undo").clicked() && !self.history.is_empty() {
                    self.undo();
                }
                if ui.button("Hint").clicked() {
                    self.hint();
                }
                if ui.button("Auto Solve").clicked() {
                    self.generate_solution();
                    self.auto_solving = true;
                    self.auto_step = 0;
                    self.play_sound("click");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("Moves: {}", self.move_count));
                });
            });

            ui.separator();

            let num_pegs = self.level.num_pegs;
            let num_disks = self.level.num_disks;
            let avail_w = ui.available_width();
            let peg_w = avail_w / (num_pegs + 1) as f32;
            let max_disk_w = peg_w * 0.85;

            ui.horizontal(|ui| {
                for i in 0..num_pegs {
                    let rect = ui
                        .allocate_exact_size(egui::vec2(peg_w, 440.0), egui::Sense::hover())
                        .1
                        .rect;
                    let painter = ui.painter_at(rect);

                    // Base
                    let base_rect = egui::Rect::from_min_max(
                        rect.min + egui::vec2(-peg_w * 0.45, 400.0),
                        rect.min + egui::vec2(peg_w * 0.45, 410.0),
                    );
                    painter.rect_filled(base_rect, 2.0, egui::Color32::DARK_GRAY);

                    // Pole
                    painter.line_segment(
                        [
                            rect.center_top() + egui::vec2(0.0, 40.0),
                            rect.center_top() + egui::vec2(0.0, 400.0),
                        ],
                        egui::Stroke::new(6.0, egui::Color32::GRAY),
                    );

                    // Static disks
                    for (level, &size) in self.pegs[i].iter().enumerate() {
                        let width = (size as f32 / num_disks as f32) * max_disk_w;
                        let y = 380.0 - (level as f32 * 36.0);

                        let disk_rect = egui::Rect::from_center_size(
                            rect.min + egui::vec2(0.0, y),
                            egui::vec2(width, 32.0),
                        );

                        let hue = (size as f32 * 40.0) % 360.0;
                        let color = egui::Color32::from_rgb(
                            (hue.sin() * 127.0 + 128.0) as u8,
                            ((hue + 120.0).sin() * 127.0 + 128.0) as u8,
                            ((hue + 240.0).sin() * 127.0 + 128.0) as u8,
                        );

                        painter.rect_filled(disk_rect, 6.0, color);
                        painter.rect_stroke(
                            disk_rect,
                            6.0,
                            egui::Stroke::new(2.0, egui::Color32::BLACK),
                            egui::StrokeKind::Outside,
                        );
                    }

                    // Animated disk
                    if let Some(anim) = &self.animated_disk {
                        if anim.from_peg == i || anim.to_peg == i {
                            let p = anim.progress.clamp(0.0, 1.0);
                            let from_x = anim.from_peg as f32 * peg_w;
                            let to_x = anim.to_peg as f32 * peg_w;
                            let cur_x = from_x + (to_x - from_x) * p;
                            // x relative to this peg's rect
                            let x = cur_x - i as f32 * peg_w;
                            let arc_y = (p * std::f32::consts::PI).sin() * 90.0;
                            let y = 220.0 - 140.0 - arc_y;

                            let width = (anim.disk as f32 / num_disks as f32) * max_disk_w;
                            let disk_rect = egui::Rect::from_center_size(
                                rect.min + egui::vec2(x, y),
                                egui::vec2(width, 32.0),
                            );

                            painter.rect_filled(disk_rect, 8.0, egui::Color32::from_rgb(255, 220, 100));
                            painter.rect_stroke(
                                disk_rect,
                                8.0,
                                egui::Stroke::new(3.0, egui::Color32::YELLOW),
                                egui::StrokeKind::Outside,
                            );
                        }
                    }

                    // Peg label — letter for A–Z, then "P27" style beyond that
                    let label = if i < 26 {
                        ((b'A' + i as u8) as char).to_string()
                    } else {
                        format!("P{}", i + 1)
                    };

                    painter.text(
                        rect.min + egui::vec2(peg_w / 2.0, 425.0),
                        egui::Align2::CENTER_TOP,
                        label,
                        egui::FontId::proportional(18.0),
                        egui::Color32::WHITE,
                    );
                }
            });

            // Hint
            if self.show_hint && !self.solution.is_empty() {
                if let Some(&(from, to)) = self.solution.get(self.move_count) {
                    let peg_label = |i: usize| -> String {
                        if i < 26 { ((b'A' + i as u8) as char).to_string() }
                        else { format!("P{}", i + 1) }
                    };
                    ui.colored_label(
                        egui::Color32::LIGHT_BLUE,
                        format!("💡 Hint: Move from {} → {}", peg_label(from), peg_label(to)),
                    );
                }
            }

            if self.is_solved() {
                ui.colored_label(egui::Color32::GOLD, "🎉 Congratulations! You solved it!");
            }
        });

        // Tick animation
        if let Some(anim) = &mut self.animated_disk {
            anim.progress += 0.085;
            if anim.progress >= 1.0 {
                self.animated_disk = None;
            }
            ctx.request_repaint();
        }

        // Auto-solve tick
        if self.auto_solving && self.animated_disk.is_none() {
            if self.auto_step < self.solution.len() {
                let (from, to) = self.solution[self.auto_step];
                self.make_move(from, to);
                self.auto_step += 1;
                ctx.request_repaint_after(std::time::Duration::from_millis(380));
            } else {
                self.auto_solving = false;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1150.0, 720.0])
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "Tower of Hanoi",
        options,
        Box::new(|_cc| Ok(Box::new(HanoiApp::default()))),
    )
}
