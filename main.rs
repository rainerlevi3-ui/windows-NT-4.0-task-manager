use eframe::egui;
use sysinfo::System;
use std::time::{Instant, Duration};
use std::process::Command;

#[derive(PartialEq)]
enum ActiveTab {
    Performance,
    Processes,
}

struct ProcessInfo {
    pid: String,
    name: String,
    memory_mb: u64,
}

struct TaskManagerApp {
    sys: System,
    last_update: Instant,
    cpu_usage: f32,
    cpu_freq_ghz: f64,
    ram_used_mb: u64,
    ram_total_mb: u64,
    gpu_name: String,
    gpu_usage: f32,
    active_tab: ActiveTab,
    cached_processes: Vec<ProcessInfo>,
}

impl Default for TaskManagerApp {
    fn default() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        
        let cpu_usage = sys.global_cpu_info().cpu_usage();
        let cpu_freq_ghz = sys.cpus().first().map(|c| c.frequency() as f64 / 1000.0).unwrap_or(0.0);
        let total_mb = sys.total_memory() / 1024 / 1024;
        let used_mb = sys.used_memory() / 1024 / 1024;

        let gpu_name = query_gpu_name();
        let gpu_usage = query_gpu_usage();
        
        let mut cached_processes = Vec::new();
        for (pid, process) in sys.processes() {
            cached_processes.push(ProcessInfo {
                pid: pid.to_string(),
                name: process.name().to_string(),
                memory_mb: process.memory() / 1024 / 1024,
            });
        }
        cached_processes.sort_by(|a, b| b.memory_mb.cmp(&a.memory_mb));

        Self {
            sys,
            last_update: Instant::now(),
            cpu_usage,
            cpu_freq_ghz,
            ram_used_mb: used_mb,
            ram_total_mb: total_mb,
            gpu_name,
            gpu_usage,
            active_tab: ActiveTab::Performance,
            cached_processes,
        }
    }
}

fn query_gpu_name() -> String {
    let output = Command::new("powershell")
        .args(&["-NoProfile", "-Command", "(Get-CimInstance Win32_VideoController).Name"])
        .output();

    match output {
        Ok(out) => {
            let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if name.is_empty() {
                "Intel Graphics Adapter".to_string()
            } else {
                name.lines().next().unwrap_or(&name).to_string()
            }
        }
        Err(_) => "Intel Integrated Graphics".to_string(),
    }
}

fn query_gpu_usage() -> f32 {
    let script = "
        $counter = Get-Counter '\\GPU Engine(*engtype_3D*)\\Utilization Percentage' -ErrorAction SilentlyContinue
        if ($counter) {
            $val = ($counter.CounterSamples | Measure-Object -Property CookedValue -Sum).Sum
            if ($val -ne $null) { [math]::Round($val, 1) } else { 0.0 }
        } else {
            0.0
        }
    ";

    let output = Command::new("powershell")
        .args(&["-NoProfile", "-Command", script])
        .output();

    match output {
        Ok(out) => {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            s.parse::<f32>().unwrap_or(0.0)
        }
        Err(_) => 0.0,
    }
}

impl eframe::App for TaskManagerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.last_update.elapsed() >= Duration::from_secs(1) {
            self.sys.refresh_cpu_usage();
            self.sys.refresh_memory();
            self.sys.refresh_processes();
            
            self.cpu_usage = self.sys.global_cpu_info().cpu_usage();
            if let Some(c) = self.sys.cpus().first() {
                self.cpu_freq_ghz = c.frequency() as f64 / 1000.0;
            }
            self.ram_total_mb = self.sys.total_memory() / 1024 / 1024;
            self.ram_used_mb = self.sys.used_memory() / 1024 / 1024;
            self.gpu_usage = query_gpu_usage();
            
            self.cached_processes.clear();
            for (pid, process) in self.sys.processes() {
                self.cached_processes.push(ProcessInfo {
                    pid: pid.to_string(),
                    name: process.name().to_string(),
                    memory_mb: process.memory() / 1024 / 1024,
                });
            }
            self.cached_processes.sort_by(|a, b| b.memory_mb.cmp(&a.memory_mb));

            self.last_update = Instant::now();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Tab Selection Header Bar
            ui.horizontal(|ui| {
                if ui.selectable_label(self.active_tab == ActiveTab::Performance, "Performance").clicked() {
                    self.active_tab = ActiveTab::Performance;
                }
                if ui.selectable_label(self.active_tab == ActiveTab::Processes, "Processes").clicked() {
                    self.active_tab = ActiveTab::Processes;
                }
            });
            ui.separator();

            match self.active_tab {
                ActiveTab::Performance => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // CPU Card
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.strong("CPU Performance");
                            ui.separator();
                            let cpu_brand = self.sys.cpus().first().map(|c| c.brand()).unwrap_or("Unknown CPU");
                            ui.label(format!("Model: {}", cpu_brand));
                            ui.label(format!("Speed: {:.2} GHz", self.cpu_freq_ghz));
                            ui.label(format!("Logical Cores: {}", self.sys.cpus().len()));
                            ui.label(format!("CPU Usage: {:.1}%", self.cpu_usage));
                        });

                        ui.add_space(8.0);

                        // System Stats Card
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.strong("System Totals");
                            ui.separator();
                            ui.label(format!("Running Processes: {}", self.cached_processes.len()));
                            let uptime_sec = System::uptime();
                            let hours = uptime_sec / 3600;
                            let minutes = (uptime_sec % 3600) / 60;
                            let seconds = uptime_sec % 60;
                            ui.label(format!("Up Time: {:02}:{:02}:{:02}", hours, minutes, seconds));
                        });

                        ui.add_space(8.0);

                        // RAM Card
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.strong("Memory (RAM)");
                            ui.separator();
                            ui.label(format!("Total Physical RAM: {} MB", self.ram_total_mb));
                            ui.label(format!("Used RAM: {} MB", self.ram_used_mb));
                            let ram_pct = if self.ram_total_mb > 0 {
                                (self.ram_used_mb as f64 / self.ram_total_mb as f64) * 100.0
                            } else {
                                0.0
                            };
                            ui.label(format!("Memory Usage: {:.1}%", ram_pct));
                        });

                        ui.add_space(8.0);

                        // GPU Card
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.strong("Graphics Processor (GPU)");
                            ui.separator();
                            ui.label(format!("Adapter: {}", self.gpu_name));
                            ui.label(format!("GPU 3D Usage: {:.1}%", self.gpu_usage));
                        });
                    });
                }
                ActiveTab::Processes => {
                    ui.heading("Running Processes");
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.strong("PID");
                        ui.add_space(60.0);
                        ui.strong("Process Name");
                        ui.add_space(150.0);
                        ui.strong("Memory Usage");
                    });
                    ui.separator();

                    let row_height = 22.0;
                    let num_rows = self.cached_processes.len();

                    egui::ScrollArea::vertical().show_rows(ui, row_height, num_rows, |ui, row_range| {
                        for i in row_range {
                            let p = &self.cached_processes[i];
                            ui.horizontal(|ui| {
                                ui.label(&p.pid);
                                ui.add_space(45.0);
                                ui.label(&p.name);
                                ui.add_space(100.0);
                                ui.label(format!("{} MB", p.memory_mb));
                            });
                        }
                    });
                }
            }
        });

        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "NT 4 Task Manager - Dynamic",
        options,
        Box::new(|_cc| Ok(Box::new(TaskManagerApp::default()))),
    )
}