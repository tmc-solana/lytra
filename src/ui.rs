use chrono::Local;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::Terminal;
use serde_json::Value;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use std::error::Error;
use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use tokio::time::{self, sleep, Duration};
use tui_logger::{TuiLoggerWidget, TuiWidgetState};

use crate::{tasks, State};

#[derive(Clone, Debug)]
pub struct UserInfo {
    pub username: String,
    pub last_tweet: String,
    pub status: String,
}

pub struct WalletInfo {
    pub balance: u64,
    pub owned_tokens: Vec<(String, String, String, String, String, f64)>,
}

pub struct StatefulTable<T> {
    state: TableState,
    items: Vec<T>,
}

impl<T> StatefulTable<T> {
    fn with_items(items: Vec<T>) -> Self {
        StatefulTable {
            state: TableState::default(),
            items,
        }
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if self.items.len() == 0 || i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    if self.items.len() == 0 {
                        0
                    } else {
                        self.items.len() - 1
                    }
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
}

pub async fn run_ui(
    keypair: Keypair,
    state: State,
    receiver: Receiver<Vec<UserInfo>>,
    rpc_client: Arc<RpcClient>,
) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut interval = time::interval(Duration::from_millis(100));
    let mut user_data: Vec<UserInfo> = vec![];

    let (tx, rx): (Sender<WalletInfo>, Receiver<WalletInfo>) = mpsc::channel();
    let pubkey = keypair.pubkey();
    let mut wallet_info_state = WalletInfo {
        balance: 0,
        owned_tokens: vec![],
    };
    let mut stateful_wallet_table =
        StatefulTable::with_items(wallet_info_state.owned_tokens.clone());

    let mut show_confirmation = false;
    let confirmation_message = "Are you sure you want to sell? (y/n)";

    let cloned_state = state.clone();
    tokio::task::spawn(async move {
        loop {
            let b = rpc_client.get_balance(&pubkey).await.unwrap();
            let owned = get_owned_tokens(pubkey.to_string()).await.unwrap();
            check_auto_sell(owned.clone(), cloned_state.clone());
            tx.send(WalletInfo {
                balance: b,
                owned_tokens: owned,
            })
            .unwrap();
            sleep(Duration::from_secs(5)).await;
        }
    });

    loop {
        interval.tick().await;
        terminal.draw(|f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
                .split(size);

            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
                .split(chunks[0]);

            let main_block = Block::default()
                .title("lytra v1.0.3")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White).bg(Color::Black));

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
                    Constraint::Percentage(5),
                    Constraint::Percentage(10),
                    Constraint::Percentage(45),
                    Constraint::Percentage(40),
                ],
            );

            let table = table
                .header(Row::new(vec![
                    Cell::from(Span::styled("Task", Style::default().fg(Color::Yellow))),
                    Cell::from(Span::styled("Username", Style::default().fg(Color::Yellow))),
                    Cell::from(Span::styled(
                        "Last Tweet",
                        Style::default().fg(Color::Yellow),
                    )),
                    Cell::from(Span::styled("Status", Style::default().fg(Color::Yellow))),
                ]))
                .block(main_block);

            f.render_widget(table, left_chunks[0]);

            let filter_state = TuiWidgetState::new()
                .set_default_display_level(log::LevelFilter::Off)
                .set_level_for_target("app", log::LevelFilter::Debug);
            // .set_level_for_target("background-task", LevelFilter::Info);
            let logs_widget = TuiLoggerWidget::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("LOGS")
                        .style(Style::default().fg(Color::White).bg(Color::Black)),
                )
                .output_separator('|')
                .output_timestamp(Some("%F %H:%M:%S%.3f".to_string()))
                .output_level(Some(tui_logger::TuiLoggerLevelOutput::Long))
                .style_error(Style::default().fg(Color::Red))
                .style_debug(Style::default().fg(Color::Green))
                .style_warn(Style::default().fg(Color::Yellow))
                .style_trace(Style::default().fg(Color::Magenta))
                .style_info(Style::default().fg(Color::Cyan))
                .output_target(false)
                .output_file(false)
                .output_line(false)
                .state(&filter_state);

            f.render_widget(logs_widget, left_chunks[1]);

            // log::info!(target:"app", "TEEEEEEEEEEEEST");
            // log::error!(target:"app", "PPPPPPPPPPPPPPP");

            // Split the Wallet Info Block into three areas
            let wallet_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Percentage(20),
                        Constraint::Percentage(60),
                        Constraint::Percentage(20),
                    ]
                    .as_ref(),
                )
                .split(chunks[1]);

            let current_time = Local::now();
            let wallet_info = Text::from(vec![
                Line::from(format!("Public Key: {}", keypair.pubkey())),
                Line::from(format!(
                    "SOL Balance: {} SOL",
                    wallet_info_state.balance as f64 / 1_000_000_000.0
                )),
                Line::from(format!(
                    "Current Time: {}",
                    current_time.format("%Y-%m-%d %H:%M:%S")
                )),
                Line::from(""),
            ]);
            let paragraph_wallet = Paragraph::new(wallet_info)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::White).bg(Color::Black)),
                )
                .wrap(ratatui::widgets::Wrap { trim: true });
            f.render_widget(paragraph_wallet, wallet_chunks[0]);

            // Adding the Wallet Table
            let wallet_info_table: Vec<Row> = stateful_wallet_table
                .items
                .iter()
                .enumerate()
                .map(|(i, token)| {
                    let style = if Some(i) == stateful_wallet_table.state.selected() {
                        Style::default()
                            .bg(Color::Blue)
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    Row::new(vec![
                        Cell::from(Span::raw(token.0.clone())).style(style),
                        Cell::from(Span::raw(token.1.clone())).style(style),
                        Cell::from(Span::raw(token.2.clone())).style(style),
                        Cell::from(Span::raw(token.3.clone())).style(style),
                    ])
                })
                .collect();
            let wallet_table = Table::new(
                wallet_info_table,
                vec![
                    Constraint::Percentage(20),
                    Constraint::Percentage(30),
                    Constraint::Percentage(30),
                    Constraint::Percentage(20),
                ],
            )
            .header(Row::new(vec![
                Cell::from(Span::styled("Symbol", Style::default().fg(Color::Yellow))),
                Cell::from(Span::styled(
                    "Initial (SOL)",
                    Style::default().fg(Color::Yellow),
                )),
                Cell::from(Span::styled(
                    "Current (SOL)",
                    Style::default().fg(Color::Yellow),
                )),
                Cell::from(Span::styled("%", Style::default().fg(Color::Yellow))),
            ]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White).bg(Color::Black)),
            );

            f.render_stateful_widget(
                wallet_table,
                wallet_chunks[1],
                &mut stateful_wallet_table.state,
            );

            // Help Menu
            let help_message = Text::from(vec![
                Line::from("Press 'q' to quit"),
                Line::from("Press 's' to sell selection"),
                Line::from("Use Up/Down arrows to navigate"),
            ]);
            let help_paragraph = Paragraph::new(help_message)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::White).bg(Color::Black)),
                )
                .alignment(Alignment::Left);
            f.render_widget(help_paragraph, wallet_chunks[2]);

            if show_confirmation {
                let popup_layout = centered_rect(60, 20, size);
                let confirmation_paragraph = Paragraph::new(Text::from(confirmation_message))
                    .block(
                        Block::default()
                            .title("Confirmation")
                            .borders(Borders::ALL)
                            .style(Style::default().fg(Color::Red).bg(Color::Black)),
                    )
                    .alignment(Alignment::Center);
                f.render_widget(confirmation_paragraph, popup_layout);
            }
        })?;

        if let Ok(new_user_data) = receiver.try_recv() {
            tracing::info!("got: {new_user_data:#?}");
            user_data = new_user_data;
            tracing::info!("new user: {user_data:#?}");
        }

        if let Ok(new_state) = rx.try_recv() {
            wallet_info_state = new_state;
            stateful_wallet_table.items = wallet_info_state.owned_tokens.clone();
        }

        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Down => stateful_wallet_table.next(),
                    KeyCode::Up => stateful_wallet_table.previous(),
                    KeyCode::Char('s') => {
                        show_confirmation = true;
                    }
                    KeyCode::Char('y') => {
                        if show_confirmation {
                            show_confirmation = false;
                            let cloned_state = state.clone();
                            let i = stateful_wallet_table.state.selected().unwrap();
                            let row = wallet_info_state.owned_tokens.get(i).unwrap().clone();
                            tokio::spawn(async move {
                                let token = row.4.clone();
                                let amount = row.5.clone();
                                tasks::sell_token_task(token, amount, cloned_state)
                                    .await
                                    .unwrap();
                            });
                        }
                    }
                    KeyCode::Char('n') => {
                        if show_confirmation {
                            show_confirmation = false;
                        }
                    }
                    _ => {}
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

pub async fn get_owned_tokens(
    public_key: String,
) -> Result<Vec<(String, String, String, String, String, f64)>, Box<dyn Error>> {
    let mut owned_tokens: Vec<(String, String, String, String, String, f64)> = vec![];

    let response = reqwest::Client::builder()
        .build()?
        .get(format!("https://wallet-api.solflare.com/v3/portfolio/tokens/{public_key}?network=mainnet&currency=USD"))
        .header("Accept", "application/json")
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::OK {
        let res: Value = response.json().await?;

        if let Some(tokens) = res["tokens"].as_array() {
            for token in tokens {
                let symbol = token["symbol"].as_str().unwrap().to_string();
                if symbol == "SOL" {
                    continue;
                }
                let token_amount = token["totalUiAmount"].as_f64().unwrap();
                if token_amount < 0.0000001 {
                    continue;
                }

                let mint = token["mint"].as_str().unwrap().to_string();
                let price_sol = token["solPrice"]["price"].as_f64().unwrap_or(0.0);
                let current_sol_worth = token_amount * price_sol;
                let initial_investment = 0.01;

                let profit_loss: f64 = if initial_investment > 0.0 {
                    ((current_sol_worth / initial_investment) - 1.0) * 100.0
                } else {
                    0.0
                };

                owned_tokens.push((
                    symbol,
                    format!("{initial_investment:.5}"),
                    format!("{current_sol_worth:.5}"),
                    format!("{profit_loss:.2}"),
                    mint,
                    token_amount,
                ));
            }
        }
    }

    Ok(owned_tokens)
}

pub fn check_auto_sell(owned: Vec<(String, String, String, String, String, f64)>, state: State) {
    if state.config.sell_config.auto_sell {
        for token in owned {
            if token.3.parse::<f64>().unwrap() >= state.config.sell_config.sell_at {
                let cloned_state = state.clone();
                tokio::spawn(async move {
                    let t = token.4.clone();
                    let amount = token.5.clone();
                    tasks::sell_token_task(t, amount, cloned_state)
                        .await
                        .unwrap();
                });
            }
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}
