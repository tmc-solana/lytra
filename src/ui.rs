use chrono::Local;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Terminal;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use std::io;
use std::sync::{Arc, Mutex};
use tokio::time::{self, Duration};

use crate::config::Config;
use crate::tasks::UserInfo;

pub async fn run_ui(
    keypair: Keypair,
    balance: u64,
    user_data: Arc<Mutex<Vec<UserInfo>>>,
    config: Config,
) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // let mut show_popup = true;
    let mut show_popup = false;
    let mut interval = time::interval(Duration::from_millis(100));

    loop {
        interval.tick().await;
        terminal.draw(|f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
                .split(size);

            let main_block = Block::default()
                .title("lytra v1.0.0")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White).bg(Color::Black));

            let user_data = user_data.lock().unwrap();
            let rows: Vec<Row> = user_data
                .iter()
                .enumerate()
                .map(|(i, user)| {
                    Row::new(vec![
                        Cell::from(Span::raw(format!("{}", i + 1))),
                        Cell::from(Span::raw(user.username.clone())),
                        Cell::from(Span::raw(user.last_tweet.clone())),
                        Cell::from(Span::raw(user.status.clone())),
                    ])
                })
                .collect();

            let table = Table::new(
                rows,
                vec![
                    Constraint::Percentage(10),
                    Constraint::Percentage(10),
                    Constraint::Percentage(40),
                    Constraint::Percentage(40),
                ],
            );

            let table = table
                .header(Row::new(vec![
                    Cell::from(Span::styled(
                        "Task Number",
                        Style::default().fg(Color::Yellow),
                    )),
                    Cell::from(Span::styled("Username", Style::default().fg(Color::Yellow))),
                    Cell::from(Span::styled(
                        "Last Tweet",
                        Style::default().fg(Color::Yellow),
                    )),
                    Cell::from(Span::styled("Status", Style::default().fg(Color::Yellow))),
                ]))
                .block(main_block);

            f.render_widget(table, chunks[0]);

            // Wallet Info Block
            let wallet_info_block = Block::default()
                .title("Wallet Info")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White).bg(Color::Black));
            let wallet_info_area = wallet_info_block.inner(chunks[1]);
            f.render_widget(wallet_info_block, chunks[1]);

            let current_time = Local::now();
            let wallet_info = Text::from(vec![
                Line::from(format!("Public Key: {}", keypair.pubkey())),
                Line::from(format!(
                    "SOL Balance: {} SOL",
                    balance as f64 / 1_000_000_000.0
                )),
                Line::from(format!(
                    "Current Time: {}",
                    current_time.format("%Y-%m-%d %H:%M:%S")
                )),
            ]);
            let paragraph_wallet = Paragraph::new(wallet_info)
                .block(Block::default().borders(Borders::NONE))
                .wrap(ratatui::widgets::Wrap { trim: true });
            f.render_widget(paragraph_wallet, wallet_info_area);

            // Config Popup
            if show_popup {
                let popup_block = Block::default()
                    .title("Config Info")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White).bg(Color::Black));
                let popup_area = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(
                        [
                            Constraint::Percentage(20),
                            Constraint::Percentage(60),
                            Constraint::Percentage(20),
                        ]
                        .as_ref(),
                    )
                    .split(size);

                let config_info = Text::from(vec![
                    Line::from(format!("RPC URL: {}", config.rpc_url)),
                    Line::from(format!("Users: {:?}", config.users)),
                ]);
                let paragraph_config = Paragraph::new(config_info).block(popup_block);
                f.render_widget(paragraph_config, popup_area[1]);
            }
        })?;

        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    if show_popup {
                        show_popup = false;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
