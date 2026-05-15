use eframe::egui;
use rodio::{OutputStream, Sink};
use std::collections::VecDeque;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Peg {
    A,
    B,
    C,
}

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

struct HanoiApp {
    num_disks: usize,
    pegs: [VecDeque<usize>; 3],
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
            num_disks: 4,
            pegs: Default::default(),
            history: Vec::new(),
            selected: None,
            animated_disk: None,
            move_count: 0,
            auto_solving: false,
            auto_step: 0,
            solution: Vec::new(),
            show_hint: false,
            _stream: _stream,
            sink: sink.map(Arc::new),
        };
        app.reset();
        app
    }
}

impl HanoiApp {
    fn reset(&mut self) {
        self.pegs = Default::default();
        for i in (1..=self.num_disks).rev() {
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
        let f = from as usize;
        let t = to as usize;

        let disk = match self.pegs[f].back() {
            Some(&d) => d,
            None => return false,
        };

        match self.pegs[t].back() {
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

        let f = from as usize;
        let disk = self.pegs[f].pop_back().unwrap();
        self.pegs[to as usize].push_back(disk);

        self.history.push(Move { from, to, disk });
        self.move_count += 1;

        // Start smooth animation
        self.animated_disk = Some(AnimatedDisk {
            disk,
            from_peg: from,
            to_peg: to,
            progress: 0.0,
        });

        self.play_sound("move");

        if self.pegs[2].len() == self.num_disks {
            self.play_sound("win");
        }
        true
    }

    fn undo(&mut self) {
        if let Some(last) = self.history.pop() {
            let disk = self.pegs[last.to as usize].pop_back().unwrap();
            self.pegs[last.from as usize].push_back(disk);
            self.move_count = self.move_count.saturating_sub(1);
            self.animated_disk = None;
            self.play_sound("move");
        }
    }

    fn generate_solution(&mut self) {
        self.solution.clear();
        fn hanoi(n: usize, src: Peg, dst: Peg, aux: Peg, moves: &mut Vec<(Peg, Peg)>) {
            if n == 1 {
                moves.push((src, dst));
                return;
            }
            hanoi(n - 1, src, aux, dst, moves);
            moves.push((src, dst));
            hanoi(n - 1, aux, dst, src, moves);
        }
        hanoi(self.num_disks, Peg::A, Peg::C, Peg::B, &mut self.solution);
    }

    fn hint(&mut self) {
        if self.solution.is_empty() {
            self.generate_solution();
        }
        self.show_hint = true;
        self.play_sound("hint");
    }
}

impl eframe::App for HanoiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🗼 Tower of Hanoi");
            ui.separator();

            // Controls
            ui.horizontal(|ui| {
                ui.label("Disks:");
                if ui.button("−").clicked() && self.num_disks > 3 {
                    self.num_disks -= 1;
                    self.reset();
                }
                ui.label(self.num_disks.to_string());
                if ui.button("+").clicked() && self.num_disks < 8 {
                    self.num_disks += 1;
                    self.reset();
                }

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

            let peg_names = ["A", "B", "C"];
            let avail_w = ui.available_width();
            let peg_w = avail_w / 4.0;
            let max_disk_w = peg_w * 0.85;

            ui.horizontal(|ui| {
                for (i, &peg) in [Peg::A, Peg::B, Peg::C].iter().enumerate() {
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
                        egui::Stroke::new(10.0, egui::Color32::GRAY),
                    );

                    // Static disks
                    for (level, &size) in self.pegs[i].iter().enumerate() {
                        let width = (size as f32 / self.num_disks as f32) * max_disk_w;
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
                        painter.rect_stroke(disk_rect, 6.0, egui::Stroke::new(3.0, egui::Color32::BLACK));
                    }

                    // Animated moving disk
                    if let Some(anim) = &self.animated_disk {
                        if (anim.from_peg as usize == i) || (anim.to_peg as usize == i) {
                            let p = anim.progress.clamp(0.0, 1.0);
                            let start_x = (anim.from_peg as usize as f32 - i as f32) * peg_w;
                            let x = start_x * (1.0 - p);
                            let y = 140.0 + (p * std::f32::consts::PI).sin() * 90.0; // parabolic arc

                            let width = (anim.disk as f32 / self.num_disks as f32) * max_disk_w;
                            let disk_rect = egui::Rect::from_center_size(
                                rect.min + egui::vec2(x, 220.0 - y),
                                egui::vec2(width, 32.0),
                            );

                            let color = egui::Color32::from_rgb(255, 220, 100);
                            painter.rect_filled(disk_rect, 8.0, color);
                            painter.rect_stroke(disk_rect, 8.0, egui::Stroke::new(4.0, egui::Color32::YELLOW));
                        }
                    }

                    // Peg label
                    painter.text(
                        rect.min + egui::vec2(peg_w / 2.0, 425.0),
                        egui::Align2::CENTER_TOP,
                        peg_names[i],
                        egui::FontId::proportional(26.0),
                        egui::Color32::WHITE,
                    );
                }
            });

            // Hint
            if self.show_hint && !self.solution.is_empty() {
                if let Some((from, to)) = self.solution.get(self.move_count) {
                    let from_str = match from { Peg::A => "A", Peg::B => "B", Peg::C => "C" };
                    let to_str = match to { Peg::A => "A", Peg::B => "B", Peg::C => "C" };
                    ui.colored_label(
                        egui::Color32::LIGHT_BLUE,
                        format!("💡 Hint: Move from {} → {}", from_str, to_str),
                    );
                }
            }

            if self.pegs[2].len() == self.num_disks {
                ui.colored_label(egui::Color32::GOLD, "🎉 Congratulations! You solved it!");
            }
        });

        // Update animation
        if let Some(anim) = &mut self.animated_disk {
            anim.progress += 0.085;
            if anim.progress >= 1.0 {
                self.animated_disk = None;
            }
            ctx.request_repaint();
        }

        // Auto-solve
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
