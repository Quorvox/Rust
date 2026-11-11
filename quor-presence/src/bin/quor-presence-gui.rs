#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::time::Duration;

use eframe::egui;
use quor_presence::config::{self, MAX_HOURS, Profile, Profiles};
use quor_presence::presence::{Status, Worker, now_ms};

fn config_path() -> PathBuf {
    let next_to_exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("profiles.toml")));
    if let Some(p) = &next_to_exe {
        if p.exists() {
            return p.clone();
        }
    }
    let cwd = PathBuf::from("profiles.toml");
    if cwd.exists() {
        return cwd;
    }
    next_to_exe.unwrap_or(cwd)
}

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() { None } else { Some(t.to_string()) }
}

// «3 д 05:30», чтобы видно было что покажет таймер
fn fmt_hours(h: f64) -> String {
    let total_min = (h * 60.0).round() as u64;
    let (d, rest) = (total_min / 1440, total_min % 1440);
    let (hh, mm) = (rest / 60, rest % 60);
    if d > 0 { format!("{d} д {hh:02}:{mm:02}") } else { format!("{hh:02}:{mm:02}") }
}

struct App {
    path: PathBuf,
    profiles: Profiles,
    current: String,
    new_name: String,
    app_id: String,
    name: String,
    details: String,
    state: String,
    hours: f64,
    worker: Option<Worker>,
    msg: Option<(bool, String)>,
}

impl App {
    fn new() -> Self {
        let path = config_path();
        let (profiles, msg) = match config::load(&path) {
            Ok(p) => (p, None),
            Err(_) => (Profiles::new(), Some((true, "profiles.toml не найден, создай профиль ниже".to_string()))),
        };
        let current = profiles.keys().next().cloned().unwrap_or_default();
        let mut app = App {
            path,
            profiles,
            current,
            new_name: String::new(),
            app_id: String::new(),
            name: String::new(),
            details: String::new(),
            state: String::new(),
            hours: 0.0,
            worker: None,
            msg,
        };
        app.load_fields();
        app
    }

    fn load_fields(&mut self) {
        let p = self.profiles.get(&self.current).cloned().unwrap_or_default();
        self.app_id = p.app_id;
        self.name = p.name.unwrap_or_default();
        self.details = p.details.unwrap_or_default();
        self.state = p.state.unwrap_or_default();
        self.hours = p.hours.unwrap_or(0.0);
    }

    // поля формы → профиль (картинки и прочее из файла сохраняем)
    fn profile(&self) -> Profile {
        let mut p = self.profiles.get(&self.current).cloned().unwrap_or_default();
        p.app_id = self.app_id.trim().to_string();
        p.name = opt(&self.name);
        p.details = opt(&self.details);
        p.state = opt(&self.state);
        p.hours = Some(self.hours);
        p
    }

    fn start(&mut self) {
        let p = self.profile();
        let res = p.validate().and_then(|_| config::start_ms(now_ms(), self.hours));
        match res {
            Ok(start) => {
                if let Some(mut w) = self.worker.take() {
                    w.stop();
                }
                self.worker = Some(Worker::spawn(p, start));
                self.msg = Some((true, "запущено".into()));
            }
            Err(e) => self.msg = Some((false, format!("{e:#}"))),
        }
    }

    fn stop(&mut self) {
        if let Some(mut w) = self.worker.take() {
            w.stop();
            self.msg = Some((true, "статус убран".into()));
        }
    }

    fn save(&mut self) {
        if self.current.is_empty() {
            self.msg = Some((false, "сначала создай профиль".into()));
            return;
        }
        self.profiles.insert(self.current.clone(), self.profile());
        self.msg = Some(match config::save(&self.path, &self.profiles) {
            Ok(()) => (true, format!("сохранено в {}", self.path.display())),
            Err(e) => (false, format!("{e:#}")),
        });
    }

    fn add_profile(&mut self) {
        let n = self.new_name.trim().to_string();
        if n.is_empty() {
            return;
        }
        if self.profiles.contains_key(&n) {
            self.msg = Some((false, "такой профиль уже есть".into()));
            return;
        }
        // новый профиль копирует текущие поля, удобно править
        let mut p = self.profile();
        p.large_image = None;
        p.large_text = None;
        p.small_image = None;
        p.small_text = None;
        self.profiles.insert(n.clone(), p);
        self.current = n;
        self.new_name.clear();
        self.save();
    }

    fn delete_profile(&mut self) {
        if self.profiles.remove(&self.current).is_some() {
            self.current = self.profiles.keys().next().cloned().unwrap_or_default();
            self.load_fields();
            self.save();
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().request_repaint_after(Duration::from_millis(500));

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("quor-presence");
            ui.label("Свой статус в Discord с любым таймером. Нужно приложение Discord (не браузер).");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Профиль:");
                let names: Vec<String> = self.profiles.keys().cloned().collect();
                let mut changed = false;
                egui::ComboBox::from_id_salt("profile").selected_text(self.current.as_str()).show_ui(ui, |ui| {
                    for n in &names {
                        if ui.selectable_value(&mut self.current, n.clone(), n).clicked() {
                            changed = true;
                        }
                    }
                });
                if changed {
                    self.load_fields();
                }
                if ui.button("Удалить").clicked() {
                    self.delete_profile();
                }
            });
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.new_name).hint_text("имя нового профиля").desired_width(160.0));
                if ui.button("+ Новый").clicked() {
                    self.add_profile();
                }
            });
            ui.separator();

            egui::Grid::new("fields").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                ui.label("Application ID");
                ui.add(egui::TextEdit::singleline(&mut self.app_id).desired_width(f32::INFINITY));
                ui.end_row();
                ui.label("Название");
                ui.add(
                    egui::TextEdit::singleline(&mut self.name)
                        .hint_text("необязательно, иначе имя приложения")
                        .desired_width(f32::INFINITY),
                );
                ui.end_row();
                ui.label("Первая строка");
                ui.add(egui::TextEdit::singleline(&mut self.details).desired_width(f32::INFINITY));
                ui.end_row();
                ui.label("Вторая строка");
                ui.add(egui::TextEdit::singleline(&mut self.state).desired_width(f32::INFINITY));
                ui.end_row();
                ui.label("Часов уже в игре");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut self.hours).range(0.0..=MAX_HOURS).speed(0.25).suffix(" ч"));
                    if ui.button("+1").clicked() {
                        self.hours = (self.hours + 1.0).min(MAX_HOURS);
                    }
                    if ui.button("+10").clicked() {
                        self.hours = (self.hours + 10.0).min(MAX_HOURS);
                    }
                    if ui.button("0").clicked() {
                        self.hours = 0.0;
                    }
                });
                ui.end_row();
            });
            ui.add_space(4.0);
            ui.weak(format!("Таймер покажет: {} и пойдёт дальше", fmt_hours(self.hours)));
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                let running = self.worker.is_some();
                if ui.button(if running { "Применить" } else { "Запустить" }).clicked() {
                    self.start();
                }
                if ui.add_enabled(running, egui::Button::new("Остановить")).clicked() {
                    self.stop();
                }
                if ui.button("Сохранить профиль").clicked() {
                    self.save();
                }
            });
            ui.add_space(6.0);

            match self.worker.as_ref().map(|w| w.status()) {
                Some(Status::Active) => {
                    ui.colored_label(egui::Color32::from_rgb(70, 190, 100), "● статус стоит в Discord");
                }
                Some(Status::Waiting) => {
                    ui.colored_label(egui::Color32::from_rgb(230, 180, 60), "● жду Discord (он запущен? ID верный?)");
                }
                None => {
                    ui.weak("● остановлен");
                }
            }
            if let Some((ok, text)) = &self.msg {
                let c = if *ok { egui::Color32::GRAY } else { egui::Color32::from_rgb(230, 90, 90) };
                ui.colored_label(c, text);
            }
            ui.add_space(6.0);
            ui.weak("«Применить» запускает таймер заново с введённого числа. Закроешь окно — статус пропадёт.");
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([480.0, 430.0]).with_title("quor-presence"),
        ..Default::default()
    };
    eframe::run_native("quor-presence", options, Box::new(|_cc| Ok(Box::new(App::new()))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hours_format() {
        assert_eq!(fmt_hours(20.0), "20:00");
        assert_eq!(fmt_hours(0.5), "00:30");
        assert_eq!(fmt_hours(50.25), "2 д 02:15");
    }

    #[test]
    fn empty_text_is_none() {
        assert_eq!(opt("  "), None);
        assert_eq!(opt(" hi "), Some("hi".to_string()));
    }
}
