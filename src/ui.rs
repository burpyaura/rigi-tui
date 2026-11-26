use crate::app::{App, Screen};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

fn spinner(frame: usize) -> &'static str {
    const S: [&str; 4] = ["|", "/", "-", "\\"];
    S[frame % 4]
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .split(f.area());

    let title = Line::from(vec![
        Span::styled(" Rigi TV ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(
            match app.screen {
                Screen::Phone => "Login",
                Screen::Otp => "OTP Verify",
                Screen::Series => "Series",
                Screen::Episodes => "Episodes",
                Screen::Playing => "Playing",
            },
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(
        Paragraph::new(title).block(Block::default().borders(Borders::ALL)),
        chunks[0],
    );

    match app.screen {
        Screen::Phone => draw_phone(f, app, chunks[1]),
        Screen::Otp => draw_otp(f, app, chunks[1]),
        Screen::Series => draw_series(f, app, chunks[1]),
        Screen::Episodes => draw_episodes(f, app, chunks[1]),
        Screen::Playing => draw_playing(f, app, chunks[1]),
    }

    let footer_line = footer(app);
    f.render_widget(
        Paragraph::new(footer_line).block(Block::default().borders(Borders::ALL)),
        chunks[2],
    );
}

fn status_line(app: &App) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    if app.busy {
        spans.push(Span::styled(
            format!(" {} ", spinner(app.spinner_frame)),
            Style::default().fg(Color::Yellow),
        ));
        spans.push(Span::raw(app.status.clone()));
    } else if let Some(e) = &app.error {
        spans.push(Span::styled(format!(" ! {}", e), Style::default().fg(Color::Red)));
    } else if let Some(n) = &app.notice {
        spans.push(Span::styled(format!(" ✓ {}", n), Style::default().fg(Color::Green)));
    }
    Line::from(spans)
}

fn footer(app: &App) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = vec![Span::styled(
        status_line(app).spans.into_iter().map(|s| s.content.to_string()).collect::<String>(),
        if app.error.is_some() {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        },
    )];
    if spans.is_empty() {
        spans = match app.screen {
            Screen::Phone => vec![Span::raw(" Tab: field switch · Enter: OTP bhejo · Ctrl+C: quit")],
            Screen::Otp => vec![Span::raw(" Enter: verify · r: resend · Esc: back")],
            Screen::Series => vec![Span::raw(" ↑↓: move · Enter: episodes · n/b: page · l: logout · q: quit")],
            Screen::Episodes => vec![Span::raw(" ↑↓: move · Enter: play · r: reload · Esc: back")],
            Screen::Playing => vec![Span::raw(" mpv chal raha hai — q: wapas")],
        };
    }
    Line::from(spans)
}

fn draw_phone(f: &mut Frame, app: &mut App, area: Rect) {
    let code_style = if app.otp_field_code {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let num_style = if app.otp_field_code {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Cyan)
    };
    let text = vec![
        Line::from(""),
        Line::from(Span::raw("Apna phone number daalein (jaise official app mein hota hai):")),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Dialing code: ", Style::default()),
            Span::styled(format!("[ {} ]", app.dialing_code), code_style),
            Span::raw("  Number: "),
            Span::styled(format!("[ {} ]", app.phone_input), num_style),
            Span::raw("_"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Server aapke number pe OTP bhejega.",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Login")),
        centered(area, 70, 9),
    );
}

fn draw_otp(f: &mut Frame, app: &mut App, area: Rect) {
    let dots: String = app.otp_input.chars().map(|_| '●').collect();
    let text = vec![
        Line::from(""),
        Line::from(format!("{} par OTP bheja gaya hai.", app.dialing_code)),
        Line::from(""),
        Line::from(vec![
            Span::raw(" OTP: "),
            Span::styled(dots, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("_"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "6 digit code enter karein · r = resend",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("OTP")),
        centered(area, 60, 9),
    );
}

fn draw_series(f: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app
        .series
        .iter()
        .map(|s| {
            let sub = s
                .total_episodes
                .map(|n| format!(" · {} episodes", n as i64))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(s.title.clone(), Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(sub, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();
    let mut ls = ListState::default();
    if !app.series.is_empty() {
        ls.select(Some(app.series_sel.min(app.series.len() - 1)));
    }
    let title = if app.busy {
        format!("Series {} {}", spinner(app.spinner_frame), app.status)
    } else {
        format!("Series — page {} (n/b: page)", app.series_page + 1)
    };
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .highlight_symbol("> ");
    f.render_stateful_widget(list, area, &mut ls);
}

fn draw_episodes(f: &mut Frame, app: &mut App, area: Rect) {
    let stitle = app
        .current_series
        .as_ref()
        .map(|s| s.title.clone())
        .unwrap_or_default();
    let items: Vec<ListItem> = app
        .episodes
        .iter()
        .map(|e| {
            let num = e
                .number
                .map(|n| format!("EP{} ", n as i64))
                .unwrap_or_default();
            let (badge, style) = match e.is_unlocked {
                Some(true) => ("▶ ", Style::default().fg(Color::Green)),
                Some(false) => ("🔒 ", Style::default().fg(Color::Yellow)),
                None => ("· ", Style::default().fg(Color::DarkGray)),
            };
            ListItem::new(Line::from(vec![
                Span::styled(num, Style::default().fg(Color::Cyan)),
                Span::styled(
                    e.title.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(badge, style),
            ]))
        })
        .collect();
    let mut ls = ListState::default();
    if !app.episodes.is_empty() {
        ls.select(Some(app.ep_sel.min(app.episodes.len() - 1)));
    }
    let title = if app.busy {
        format!("{} — {} {}", stitle, spinner(app.spinner_frame), app.status)
    } else {
        format!("{} — episodes", stitle)
    };
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .highlight_symbol("> ");
    f.render_stateful_widget(list, area, &mut ls);
}

fn draw_playing(f: &mut Frame, app: &mut App, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "▶ Playing",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::raw(app.playing_title.clone())),
        Line::from(""),
        Line::from(Span::styled(
            "mpv external window mein chal raha hai. q dabao to wapas.",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(
        Paragraph::new(text).alignment(Alignment::Center).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Player"),
        ),
        centered(area, 64, 10),
    );
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect {
        x,
        y,
        width: w.min(area.width),
        height: h.min(area.height),
    }
}
