#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::egui;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

#[derive(Default, Deserialize, Serialize)]
struct Host {
    name: String,
    address: String,
    username: String,
    port: String,
}

impl Host {
    fn normalize_and_validate(&mut self) -> Result<(), &'static str> {
        self.name = self.name.trim().to_owned();
        self.address = self.address.trim().to_owned();
        self.username = self.username.trim().to_owned();
        self.port = self.port.trim().to_owned();
        if self.name.is_empty() || self.address.is_empty() || self.username.is_empty()
            || self.address.starts_with('-') || self.address.chars().any(char::is_whitespace)
            || self.username.chars().any(char::is_whitespace)
            || !matches!(self.port.parse::<u16>(), Ok(1..=65535)) {
            return Err("Enter a name, hostname, username, and a port from 1 to 65535.");
        }
        Ok(())
    }
}

struct Terminal {
    parser: Arc<Mutex<vt100::Parser>>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
}

impl Terminal {
    fn start(command: CommandBuilder, context: egui::Context) -> Result<Self, String> {
        let pair = native_pty_system()
            .openpty(PtySize { rows: 30, cols: 100, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string())?;
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let child = pair.slave.spawn_command(command).map_err(|e| e.to_string())?;
        drop(pair.slave);
        let parser = Arc::new(Mutex::new(vt100::Parser::new(30, 100, 1000)));
        let output = Arc::clone(&parser);
        std::thread::spawn(move || {
            let mut buffer = [0; 8192];
            while let Ok(count) = reader.read(&mut buffer) {
                if count == 0 { break; }
                if let Ok(mut parser) = output.lock() { parser.process(&buffer[..count]); }
                context.request_repaint();
            }
        });
        Ok(Self { parser, writer, child, _master: pair.master })
    }
}

impl Drop for Terminal {
    fn drop(&mut self) { let _ = self.child.kill(); let _ = self.child.wait(); }
}

struct Harbor {
    hosts: Vec<Host>,
    draft: Host,
    terminal: Option<Terminal>,
    title: String,
    input: String,
    error: String,
    screenshot: Option<std::path::PathBuf>,
    frames: u32,
}

impl Harbor {
    fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<std::path::PathBuf>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        Self {
            hosts: cc.storage.and_then(|s| eframe::get_value(s, "hosts")).unwrap_or_default(),
            draft: Host { port: "22".into(), ..Default::default() },
            terminal: None, title: "Welcome to Harbor".into(), input: String::new(), error: String::new(),
            screenshot, frames: 0,
        }
    }

    fn open(&mut self, command: CommandBuilder, title: String, ctx: &egui::Context) {
        match Terminal::start(command, ctx.clone()) {
            Ok(terminal) => { self.terminal = Some(terminal); self.title = title; self.input.clear(); self.error.clear(); }
            Err(error) => self.error = format!("Could not open terminal: {error}"),
        }
    }

    fn send(&mut self, bytes: &[u8]) {
        if let Some(terminal) = &mut self.terminal {
            if let Err(error) = terminal.writer.write_all(bytes).and_then(|_| terminal.writer.flush()) {
                self.error = format!("Could not send input: {error}");
            }
        }
    }
}

impl eframe::App for Harbor {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "hosts", &self.hosts);
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if let Some(path) = &self.screenshot {
            for event in ctx.input(|input| input.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                            std::fs::create_dir_all(parent)?;
                        }
                        let pixels: Vec<u8> = image.pixels.iter().flat_map(|pixel| pixel.to_array()).collect();
                        image::save_buffer(path, &pixels, image.size[0] as u32, image.size[1] as u32, image::ColorType::Rgba8)?;
                        Ok(())
                    })();
                    if let Err(error) = result {
                        eprintln!("Could not save screenshot: {error}");
                        std::process::exit(1);
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
            }
            self.frames += 1;
            if self.frames == 120 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            }
            ctx.request_repaint();
        }
        egui::SidePanel::left("connections").default_width(260.0).show(ctx, |ui| {
            ui.heading("Harbor");
            ui.label("Your connections, close at hand");
            ui.add_space(20.0);
            if ui.button("Open local terminal").clicked() {
                #[cfg(windows)]
                let shell = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into());
                #[cfg(not(windows))]
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
                self.open(CommandBuilder::new(shell), "Local terminal".into(), ctx);
            }
            ui.separator();
            ui.heading("Hosts");
            let mut connect = None;
            let mut remove = None;
            for (index, host) in self.hosts.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.button(&host.name).clicked() { connect = Some(index); }
                    if ui.small_button("Remove").clicked() { remove = Some(index); }
                });
                ui.weak(format!("{}@{}:{}", host.username, host.address, host.port));
            }
            if let Some(index) = connect {
                // Persisted records are untrusted too: they may predate validation or
                // have been edited outside the application.
                let host = &mut self.hosts[index];
                if let Err(error) = host.normalize_and_validate() {
                    self.error = format!("Invalid saved connection: {error}");
                } else {
                    let mut command = CommandBuilder::new("ssh");
                    command.args(["-o", "StrictHostKeyChecking=ask", "-p", &host.port, "-l", &host.username, "--", &host.address]);
                    let title = host.name.clone();
                    self.open(command, title, ctx);
                }
            }
            if let Some(index) = remove { self.hosts.remove(index); }
            ui.add_space(20.0);
            ui.heading("Add a connection");
            ui.label("Name"); ui.text_edit_singleline(&mut self.draft.name);
            ui.label("Hostname or IP address"); ui.text_edit_singleline(&mut self.draft.address);
            ui.label("Username"); ui.text_edit_singleline(&mut self.draft.username);
            ui.label("Port"); ui.text_edit_singleline(&mut self.draft.port);
            if ui.button("Save connection").clicked() {
                if let Err(error) = self.draft.normalize_and_validate() {
                    self.error = error.into();
                } else {
                    self.hosts.push(std::mem::replace(&mut self.draft, Host { port: "22".into(), ..Default::default() }));
                    self.error.clear();
                }
            }
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(&self.title);
            if !self.error.is_empty() { ui.colored_label(egui::Color32::LIGHT_RED, &self.error); }
            if self.terminal.is_some() {
                ui.horizontal(|ui| {
                    if ui.button("Disconnect").clicked() {
                        self.terminal = None;
                        self.input.clear();
                        self.title = "Welcome to Harbor".into();
                    }
                    if ui.button("Ctrl+C").clicked() { self.send(&[3]); }
                    if ui.button("Tab").clicked() { self.send(&[9]); }
                });
                if let Some(terminal) = &self.terminal {
                    let contents = terminal.parser.lock().map(|p| p.screen().contents()).unwrap_or_default();
                    egui::ScrollArea::both().max_height((ui.available_height() - 70.0).max(100.0)).show(ui, |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(contents).monospace()).selectable(true));
                    });
                    ui.separator();
                    ui.label("Terminal input (hidden for passwords)");
                    ui.horizontal(|ui| {
                        let response = ui.add(egui::TextEdit::singleline(&mut self.input).password(true).desired_width(f32::INFINITY));
                        let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if ui.button("Send").clicked() || enter {
                            let input = std::mem::take(&mut self.input) + "\r";
                            self.send(input.as_bytes());
                            response.request_focus();
                        }
                    });
                }
            } else {
                ui.add_space(60.0);
                ui.heading("A home for your terminals");
                ui.label("Open a local terminal or save an SSH connection to get started.");
                ui.label("SSH connections use your system's OpenSSH client and known-host checks.");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::Host;

    #[test]
    fn normalizes_valid_host() {
        let mut host = Host { name: " Server ".into(), address: " example.org ".into(), username: " alice ".into(), port: " 22 ".into() };
        assert!(host.normalize_and_validate().is_ok());
        assert_eq!((&*host.name, &*host.address, &*host.username, &*host.port), ("Server", "example.org", "alice", "22"));
    }

    #[test]
    fn rejects_invalid_connection_fields() {
        for (address, username, port) in [("-option", "alice", "22"), ("host name", "alice", "22"), ("host", "two users", "22"), ("host", "alice", "0"), ("host", "alice", "65536")] {
            let mut host = Host { name: "Server".into(), address: address.into(), username: username.into(), port: port.into() };
            assert!(host.normalize_and_validate().is_err());
        }
    }
}

fn main() -> eframe::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let screenshot = match args.next() {
        Some(arg) if arg == "--screenshot" => match (args.next(), args.next()) {
            (Some(path), None) => Some(path.into()),
            _ => { eprintln!("Usage: harbor [--screenshot <file.png>]"); std::process::exit(2); }
        },
        Some(_) => { eprintln!("Usage: harbor [--screenshot <file.png>]"); std::process::exit(2); }
        None => None,
    };
    eframe::run_native("Harbor", eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 720.0]),
        ..Default::default()
    }, Box::new(move |cc| Ok(Box::new(Harbor::new(cc, screenshot)))))
}
