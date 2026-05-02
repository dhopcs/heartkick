//! TUI app state and logic

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::bluetooth::DeviceInfo;
use crate::config::Config;
use crate::core::{Engine, EngineEvent, EngineSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Home,
    Devices,
    Metrics,
    Logs,
    Settings,
}

impl Tab {
    pub const ALL: &'static [Tab] = &[
        Tab::Home,
        Tab::Devices,
        Tab::Metrics,
        Tab::Logs,
        Tab::Settings,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Home => "Home",
            Tab::Devices => "Devices",
            Tab::Metrics => "Metrics",
            Tab::Logs => "Logs",
            Tab::Settings => "Settings",
        }
    }
}

// App state
pub struct App {
    pub tab: Tab,
    pub snapshot: EngineSnapshot,

    // devices tab
    pub devices: Vec<DeviceInfo>,
    pub scanning: bool,
    pub selected_device: usize,
    pub device_error: Option<String>,
    pub connecting: Option<String>,

    // metrics tab
    pub bpm_history: VecDeque<u64>,

    // logs tab
    pub logs: Vec<String>,
    pub log_paused: bool,
    /// Lines scrolled up from the bottom (0 = follow tail).
    pub log_scroll: usize,

    // settings tab
    pub config: Config,
    pub config_file: PathBuf,
    pub data_dir: PathBuf,

    // engine ref
    pub engine: Arc<Engine>,
    pub should_quit: bool,
}

impl App {
    pub fn new(
        engine: Arc<Engine>,
        config: Config,
        config_file: PathBuf,
        data_dir: PathBuf,
    ) -> Self {
        let snapshot = engine.snapshot();
        Self {
            tab: Tab::Home,
            snapshot,
            devices: vec![],
            scanning: false,
            selected_device: 0,
            device_error: None,
            connecting: None,
            bpm_history: VecDeque::with_capacity(300),
            logs: vec![],
            log_paused: false,
            log_scroll: 0,
            config,
            config_file,
            data_dir,
            engine,
            should_quit: false,
        }
    }

    pub fn on_engine_event(&mut self, event: EngineEvent) {
        self.snapshot = self.engine.snapshot();
        if let EngineEvent::Sample { ref sample, .. } = event {
            self.bpm_history.push_back(sample.bpm as u64);
            if self.bpm_history.len() > 300 {
                self.bpm_history.pop_front();
            }
        }
    }

    pub fn refresh_logs(&mut self) {
        if !self.log_paused {
            self.logs = crate::logs::recent(400);
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        // global keys
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return;
            }
            KeyCode::Tab => {
                let idx = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
                self.tab = Tab::ALL[(idx + 1) % Tab::ALL.len()];
                return;
            }
            KeyCode::BackTab => {
                let idx = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
                self.tab = Tab::ALL[(idx + Tab::ALL.len() - 1) % Tab::ALL.len()];
                return;
            }
            KeyCode::Char('1') => {
                self.tab = Tab::Home;
                return;
            }
            KeyCode::Char('2') => {
                self.tab = Tab::Devices;
                return;
            }
            KeyCode::Char('3') => {
                self.tab = Tab::Metrics;
                return;
            }
            KeyCode::Char('4') => {
                self.tab = Tab::Logs;
                return;
            }
            KeyCode::Char('5') => {
                self.tab = Tab::Settings;
                return;
            }
            _ => {}
        }

        // tab specific keys
        match self.tab {
            Tab::Devices => self.on_devices_key(key),
            Tab::Logs => self.on_logs_key(key),
            _ => {}
        }
    }

    fn on_devices_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if self.selected_device > 0 => {
                self.selected_device -= 1;
            }
            KeyCode::Down | KeyCode::Char('j')
                if !self.devices.is_empty() && self.selected_device + 1 < self.devices.len() =>
            {
                self.selected_device += 1;
            }
            _ => {}
        }
    }

    fn on_logs_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('p') | KeyCode::Char('P') => {
                self.log_paused = !self.log_paused;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.log_scroll = self.log_scroll.saturating_add(3);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.log_scroll = self.log_scroll.saturating_sub(3);
            }
            KeyCode::PageUp => {
                self.log_scroll = self.log_scroll.saturating_add(20);
            }
            KeyCode::PageDown => {
                self.log_scroll = self.log_scroll.saturating_sub(20);
            }
            KeyCode::Char('g') => {
                self.log_scroll = self.logs.len();
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.log_scroll = 0;
            }
            _ => {}
        }
    }
}
