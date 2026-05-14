use eframe::egui;
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq)]
enum Peg {
    A,
    B,
    C,
}

struct HanoiApp {
    num_disks: usize,
    pegs: [VecDeque<usize>; 3],
    selected: Option<(Peg, usize)>, // (peg, disk_size)
    move_count: usize,
    auto_solving: bool,
    auto_step: usize,
    solution: Vec<(Peg, Peg)>,
}

impl Default for HanoiApp {
    fn default() -> Self {
        let mut app = Self {
            num_disks: 4,
            pegs: Default::default(),
            selected: None,
            move_count: 0,
            auto_solving: false,
            auto_step: 0,
            solution: Vec::new(),
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
        self.selected = None;
        self.move_count = 0;
        self.auto_solving = false;
        self.auto_step = 0;
        self.solution.clear();
    }

    fn is_valid_move(&self, from: Peg, to: Peg) -> bool {
        let from_idx = from as usize;
        let to_idx = to as usize;

        let disk = match self.pegs[from_idx].back() {
            Some(&d) => d,
            None => return false,
        };

        match self.pegs[to_idx].back() {
            Some(&top) => disk < top,
            None => true,
        }
    }

    fn make_move(&mut self, from: Peg, to: Peg) -> bool {
        if self.is_valid_move(from, to) {
            let from_idx = from as usize;
            let to_idx = to as usize;
            if let Some(disk) = self.pegs[from_idx].pop_back() {
                self.pegs[to_idx].push_back(disk);
                self.move_count += 1;
                return true;
            }
        }
        false
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

    fn auto_solve_step(&mut self) {
        if self.auto_step < self.solution.len() {
            let (from, to) = self.solution[self.auto_step];
            self.make_move(from, to);
            self.auto_step += 1;
        } else {
            self.auto_solving = false;
        }
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
                if ui.button("-").clicked() && self.num_disks > 3 {
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
                }

                if ui.button("Auto Solve").clicked() {
                    self.generate_solution();
                    self.auto_solving = true;
                    self.auto_step = 0;
                }

                ui.label(format!("Moves: {}", self.move_count));
            });

            ui.separator();

            let peg_names = ["A", "B", "C"];
            let available_width = ui.available_width();
            let peg_width = available_width / 4.0;
            let disk_max_width = peg_width * 0.8;

            let response = ui.horizontal(|ui| {
                for (i, peg) in [Peg::A, Peg::B, Peg::C].iter().enumerate() {
                    let peg_rect = ui.allocate_exact_size(
                        egui::vec2(peg_width, 400.0),
                        egui::Sense::hover(),
                    );

                    let painter = ui.painter_at(peg_rect.1.rect);

                    // Draw base
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            peg_rect.1.rect.min + egui::vec2(-peg_width * 0.4, 380.0),
                            peg_rect.1.rect.min + egui::vec2(peg_width * 0.4, 390.0),
                        ),
                        2.0,
                        egui::Color32::DARK_GRAY,
                    );

                    // Draw pole
                    painter.line_segment(
                        [
                            peg_rect.1.rect.min + egui::vec2(0.0, 50.0),
                            peg_rect.1.rect.min + egui::vec2(0.0, 380.0),
                        ],
                        egui::Stroke::new(8.0, egui::Color32::GRAY),
                    );

                    // Draw disks
                    let disks = &self.pegs[i];
                    for (level, &size) in disks.iter().enumerate() {
                        let width = (size as f32 / self.num_disks as f32) * disk_max_width;
                        let y = 360.0 - (level as f32 * 35.0);

                        let disk_rect = egui::Rect::from_min_max(
                            peg_rect.1.rect.min + egui::vec2(-width / 2.0, y - 30.0),
                            peg_rect.1.rect.min + egui::vec2(width / 2.0, y),
                        );

                        let color = egui::Color32::from_hsv((size as f32 * 40.0) % 360.0, 0.8, 0.9);

                        painter.rect_filled(disk_rect, 5.0, color);
                        painter.rect_stroke(disk_rect, 5.0, egui::Stroke::new(2.0, egui::Color32::BLACK));

                        // Click to select / move
                        let disk_id = egui::Id::new(("disk", i, level));
                        let sense = ui.interact(disk_rect, disk_id, egui::Sense::click_and_drag());

                        if sense.clicked() {
                            if self.selected.is_none() {
                                if let Some(&top) = disks.back() {
                                    if top == size {
                                        self.selected = Some((*peg, size));
                                    }
                                }
                            } else {
                                let (from_peg, _) = self.selected.unwrap();
                                if from_peg != *peg {
                                    if self.make_move(from_peg, *peg) {
                                        self.selected = None;
                                    }
                                } else {
                                    self.selected = None;
                                }
                            }
                        }
                    }

                    // Peg label
                    painter.text(
                        peg_rect.1.rect.min + egui::vec2(0.0, 410.0),
                        egui::Align2::CENTER_TOP,
                        peg_names[i],
                        egui::FontId::proportional(20.0),
                        egui::Color32::WHITE,
                    );
                }
            });

            // Status
            if let Some((peg, size)) = self.selected {
                ui.colored_label(egui::Color32::YELLOW, format!("Selected disk {} from peg {:?}", size, peg));
            }

            if self.auto_solving {
                ui.colored_label(egui::Color32::GREEN, "Auto-solving...");
                self.auto_solve_step();
                ctx.request_repaint_after(std::time::Duration::from_millis(300));
            }

            // Win condition
            if self.pegs[2].len() == self.num_disks {
                ui.colored_label(egui::Color32::GOLD, "🎉 Congratulations! You solved it!");
            }
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 600.0])
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "Tower of Hanoi - egui",
        options,
        Box::new(|_cc| Ok(Box::new(HanoiApp::default()))),
    )
}
