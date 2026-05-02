//! Ratatui rendering code for the TUI

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Sparkline, Tabs};
use ratatui::Frame;

use crate::bluetooth::ConnectionState;

use super::app::{App, Tab};

const PULSE: Color = Color::Rgb(220, 50, 50);

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();

    // Vertical split: tab bar | content | help bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(0),    // page content
            Constraint::Length(1), // key-binding hint
        ])
        .split(area);

    draw_tab_bar(f, app, chunks[0]);
    draw_page(f, app, chunks[1]);
    draw_help_bar(f, app, chunks[2]);
}

// tab bar

fn draw_tab_bar(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, t)| Line::from(format!(" {} {} ", i + 1, t.title())))
        .collect();

    let selected = Tab::ALL.iter().position(|t| *t == app.tab).unwrap_or(0);

    let tabs = Tabs::new(titles)
        .select(selected)
        .block(Block::default().borders(Borders::BOTTOM))
        .highlight_style(Style::default().fg(PULSE).add_modifier(Modifier::BOLD))
        .style(Style::default().fg(Color::DarkGray));

    f.render_widget(tabs, area);
}

// help bar

fn draw_help_bar(f: &mut Frame, app: &App, area: Rect) {
    let text = match app.tab {
        Tab::Home => "Tab/1-5 switch tab  q quit",
        Tab::Devices => "s scan HR  a scan all  ↑↓/jk select  Enter connect  d disconnect  q quit",
        Tab::Metrics => "r reset session  q quit",
        Tab::Logs => "p pause  ↑↓/jk scroll  PageUp/Dn  g top  G bottom  q quit",
        Tab::Settings => "q quit",
    };

    f.render_widget(
        Paragraph::new(format!(" {text} "))
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Left),
        area,
    );
}

// pages

fn draw_page(f: &mut Frame, app: &App, area: Rect) {
    match app.tab {
        Tab::Home => draw_home(f, app, area),
        Tab::Devices => draw_devices(f, app, area),
        Tab::Metrics => draw_metrics(f, app, area),
        Tab::Logs => draw_logs(f, app, area),
        Tab::Settings => draw_settings(f, app, area),
    }
}

// home

fn draw_home(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);

    draw_bpm_display(f, app, chunks[0]);
    draw_connection_status(f, app, chunks[1]);
}

fn draw_bpm_display(f: &mut Frame, app: &App, area: Rect) {
    let snap = &app.snapshot;
    let bpm_opt = snap.last_sample.as_ref().map(|s| s.bpm);

    let bpm_text = match bpm_opt {
        Some(bpm) => format!("{bpm} BPM"),
        None => "-- BPM".to_string(),
    };
    let bpm_color = if bpm_opt.is_some() {
        PULSE
    } else {
        Color::DarkGray
    };

    // Vertical centering inside the area
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1), // BPM line
            Constraint::Fill(1),
        ])
        .split(area);

    // BPM line
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "♥  ",
                Style::default().fg(PULSE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                bpm_text,
                Style::default().fg(bpm_color).add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Center),
        inner[1],
    );
}

fn draw_connection_status(f: &mut Frame, app: &App, area: Rect) {
    let snap = &app.snapshot;

    let (badge, badge_color) = match snap.state {
        ConnectionState::Disconnected => ("● Disconnected", Color::DarkGray),
        ConnectionState::Scanning => ("◌ Scanning…", Color::Yellow),
        ConnectionState::Connecting => ("◌ Connecting…", Color::Yellow),
        ConnectionState::Connected => ("● Connected", Color::Green),
    };

    let mut lines = vec![Line::from(vec![
        Span::raw("Status:  "),
        Span::styled(
            badge,
            Style::default()
                .fg(badge_color)
                .add_modifier(Modifier::BOLD),
        ),
    ])];

    if let Some(addr) = &snap.device_address {
        lines.push(Line::from(vec![
            Span::raw("Device:  "),
            Span::styled(addr.as_str(), Style::default().fg(Color::White)),
        ]));
    }

    if snap.state == ConnectionState::Disconnected {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press [2] to open Devices and connect a heart rate monitor.",
            Style::default().fg(Color::Yellow),
        )));
    }

    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Connection ")
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .style(Style::default().fg(Color::Gray)),
        area,
    );
}

// devices

fn draw_devices(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // status banner
            Constraint::Min(0),    // device list
            Constraint::Length(2), // error / hint
        ])
        .split(area);

    // status
    let (status_text, status_style) = match app.snapshot.state {
        ConnectionState::Connected => (
            format!(
                "● Connected  {}",
                app.snapshot.device_address.as_deref().unwrap_or("")
            ),
            Style::default().fg(Color::Green),
        ),
        ConnectionState::Connecting => (
            format!(
                "◌ Connecting to {}…",
                app.connecting.as_deref().unwrap_or("device")
            ),
            Style::default().fg(Color::Yellow),
        ),
        ConnectionState::Scanning => (
            "◌ Scanning…".to_string(),
            Style::default().fg(Color::Yellow),
        ),
        ConnectionState::Disconnected => (
            "● Disconnected".to_string(),
            Style::default().fg(Color::DarkGray),
        ),
    };

    f.render_widget(
        Paragraph::new(status_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .style(status_style),
        chunks[0],
    );

    // device list
    let items: Vec<ListItem> = if app.scanning {
        vec![ListItem::new(Line::from(Span::styled(
            "  Scanning for heart rate monitors (5 s)…",
            Style::default().fg(Color::Yellow),
        )))]
    } else if app.devices.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  No devices found.  Press [s] to scan for HR monitors, [a] to scan all.",
            Style::default().fg(Color::DarkGray),
        )))]
    } else {
        app.devices
            .iter()
            .map(|d| {
                let name = d.name.as_deref().unwrap_or("Unknown");
                let rssi = d.rssi.map(|r| format!("  {}dBm", r)).unwrap_or_default();
                let hr = if d.advertises_hr { "  ♥" } else { "" };
                ListItem::new(Line::from(format!(
                    "  {}  {}{}{}",
                    name, d.address, rssi, hr
                )))
            })
            .collect()
    };

    let title = format!(" Devices ({}) ", app.devices.len());
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(PULSE)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    if !app.devices.is_empty() {
        list_state.select(Some(app.selected_device));
    }

    f.render_stateful_widget(list, chunks[1], &mut list_state);

    if let Some(err) = &app.device_error {
        f.render_widget(
            Paragraph::new(format!(" Error: {err}")).style(Style::default().fg(Color::Red)),
            chunks[2],
        );
    }
}

// metrics

fn fmt_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

fn draw_metrics(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // stat tiles row
            Constraint::Min(0),    // sparkline chart
        ])
        .split(area);

    draw_stat_tiles(f, app, chunks[0]);
    draw_sparkline(f, app, chunks[1]);
}

fn draw_stat_tiles(f: &mut Frame, app: &App, area: Rect) {
    let sess = &app.snapshot.session;
    let bpm = app.snapshot.last_sample.as_ref().map(|s| s.bpm);

    let tiles: &[(&str, String, Option<Color>)] = &[
        (
            "BPM",
            bpm.map_or("--".into(), |b| b.to_string()),
            Some(PULSE),
        ),
        (
            "MIN",
            sess.min_bpm.map_or("--".into(), |v| v.to_string()),
            None,
        ),
        (
            "MAX",
            sess.max_bpm.map_or("--".into(), |v| v.to_string()),
            None,
        ),
        (
            "AVG",
            sess.avg_bpm.map_or("--".into(), |v| format!("{v:.1}")),
            None,
        ),
        ("DURATION", fmt_duration(sess.duration_secs()), None),
        (
            "RMSSD",
            app.snapshot
                .rmssd
                .map_or("--".into(), |v| format!("{v:.1} ms")),
            None,
        ),
    ];

    let n = tiles.len() as u32;
    let tile_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Ratio(1, n); tiles.len()])
        .split(area);

    for (i, (label, value, color)) in tiles.iter().enumerate() {
        let val_style = Style::default()
            .fg(color.unwrap_or(Color::White))
            .add_modifier(Modifier::BOLD);

        let content = vec![
            Line::from(Span::styled(
                *label,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            )),
            Line::from(""),
            Line::from(Span::styled(value.clone(), val_style)),
        ];

        f.render_widget(
            Paragraph::new(content)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::DarkGray)),
                )
                .alignment(Alignment::Center),
            tile_areas[i],
        );
    }
}

fn draw_sparkline(f: &mut Frame, app: &App, area: Rect) {
    let data: Vec<u64> = app.bpm_history.iter().cloned().collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" BPM History ")
        .border_style(Style::default().fg(Color::DarkGray));

    if data.is_empty() {
        f.render_widget(
            Paragraph::new("No samples yet…")
                .block(block)
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    f.render_widget(
        Sparkline::default()
            .block(block)
            .data(&data)
            .style(Style::default().fg(PULSE)),
        area,
    );
}

// logs

fn draw_logs(f: &mut Frame, app: &App, area: Rect) {
    let paused = if app.log_paused {
        " ▐▌ PAUSED "
    } else {
        ""
    };
    let title = format!(" Logs ({} lines){paused}", app.logs.len());

    let title_style = if app.log_paused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_style(title_style)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner_height = area.height.saturating_sub(2) as usize;
    let total = app.logs.len();
    // scroll_offset: 0 = show tail, N = scroll N lines up from tail
    let scroll_offset = app.log_scroll.min(total.saturating_sub(inner_height));
    let start = total.saturating_sub(inner_height + scroll_offset);

    let lines: Vec<Line> = app.logs[start..]
        .iter()
        .map(|line| {
            let color = if line.contains("ERROR") {
                Color::Red
            } else if line.contains("WARN") {
                Color::Yellow
            } else if line.contains("DEBUG") {
                Color::Blue
            } else {
                Color::Gray
            };
            Line::from(Span::styled(line.as_str(), Style::default().fg(color)))
        })
        .collect();

    f.render_widget(Paragraph::new(lines).block(block), area);
}

// settings

fn draw_settings(f: &mut Frame, app: &App, area: Rect) {
    let cfg = &app.config;

    let section = |s: &str| -> Line {
        Line::from(Span::styled(
            s.to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
    };

    let kv = |k: &str, v: &str| -> Line {
        Line::from(vec![
            Span::styled(format!("  {k:<20}"), Style::default().fg(Color::DarkGray)),
            Span::raw(v.to_string()),
        ])
    };

    let enabled = |flag: bool, extra: &str| -> String {
        if flag {
            format!("enabled  {extra}").trim_end().to_string()
        } else {
            "disabled".to_string()
        }
    };

    let lines: Vec<Line> = vec![
        section("Paths"),
        kv("Config file:", &app.config_file.display().to_string()),
        kv("Data dir:", &app.data_dir.display().to_string()),
        Line::from(""),
        section("General"),
        kv("Locale:", &cfg.general.locale),
        kv("Log level:", &cfg.general.log_level),
        Line::from(""),
        section("Bluetooth"),
        kv(
            "Saved device:",
            cfg.bluetooth.device_address.as_deref().unwrap_or("(none)"),
        ),
        kv("Auto-reconnect:", &cfg.bluetooth.auto_reconnect.to_string()),
        Line::from(""),
        section("API"),
        kv(
            "HTTP:",
            &enabled(cfg.api.http_enabled, &format!("({})", cfg.api.http_bind)),
        ),
        kv("IPC socket:", &enabled(cfg.api.socket_enabled, "")),
        kv(
            "Auth token:",
            if cfg.api.api_token.is_some() {
                "set"
            } else {
                "(none)"
            },
        ),
        Line::from(""),
        section("Integrations"),
        kv(
            "Prometheus:",
            &enabled(
                cfg.integrations.prometheus.enabled,
                &format!("({})", cfg.integrations.prometheus.bind),
            ),
        ),
        kv(
            "OSC:",
            &enabled(
                cfg.integrations.osc.enabled,
                &format!("→ {}", cfg.integrations.osc.target),
            ),
        ),
        kv(
            "Overlay:",
            &enabled(
                cfg.integrations.overlay.enabled,
                &format!("(http://{})", cfg.integrations.overlay.bind),
            ),
        ),
        kv(
            "Webhooks:",
            &format!("{} configured", cfg.integrations.webhooks.len()),
        ),
        Line::from(""),
        Line::from(Span::styled(
            "  Edit config.toml to change these settings.",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Settings ")
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .style(Style::default().fg(Color::White)),
        area,
    );
}
