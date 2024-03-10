use chrono::Local;
use iced::{
    theme,
    widget::{button, column, horizontal_space, row, scrollable, text, Column},
    Element,
};

use crate::{main_window::Error, main_window::Message};

#[derive(Debug, Clone)]
struct LogEntry {
    severity: LogSeverity,
    timestamp: String,
    message: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
enum LogSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Default)]
pub struct Logs {
    stash: Vec<LogEntry>,
}

impl Logs {
    pub fn view(&self) -> Element<Message> {
        if self.stash.is_empty() {
            return text("Сообщений нет").into();
        }

        let scrollable_column = scrollable(self.stash.iter().fold(
            Column::new().spacing(5),
            |column, log_entry| {
                column.push(
                    row![
                        text(format!(
                            "[{}]",
                            match log_entry.severity {
                                LogSeverity::Info => "INFO",
                                LogSeverity::Warning => "WARN",
                                LogSeverity::Error => "ERROR",
                            }
                        )),
                        text(&log_entry.timestamp),
                        text(&log_entry.message)
                    ]
                    .spacing(5),
                )
            },
        ));

        column![
            row![
                horizontal_space(),
                button("Очистить лог")
                    .on_press(Message::ClearLogs)
                    .style(theme::Button::Destructive)
            ]
            .spacing(5),
            scrollable_column
        ]
        .padding(5)
        .into()
    }

    #[allow(dead_code)]
    pub fn debug(&mut self, msg: &str) {
        self.stash.push(LogEntry {
            severity: LogSeverity::Info,
            timestamp: Local::now().format("%H:%M").to_string(),
            message: msg.to_string(),
        })
    }

    pub fn error(&mut self, err: Error) {
        let stamp = Local::now().format("%H:%M").to_string();
        self.stash.push({
            match err {
                Error::DialogClosed => LogEntry {
                    severity: LogSeverity::Info,
                    timestamp: stamp,
                    message: "Dialog closed".to_string(),
                },
                Error::IO(kind) => LogEntry {
                    severity: LogSeverity::Error,
                    timestamp: stamp,
                    message: format!("IO: {}", kind),
                },
                Error::Parse(msg, _) => LogEntry {
                    severity: LogSeverity::Error,
                    timestamp: stamp,
                    message: format!("Parser: {}", msg),
                },
                Error::Query(msg) => LogEntry {
                    severity: LogSeverity::Info,
                    timestamp: stamp,
                    message: format!("Search: {}", msg),
                },
            }
        });
    }

    pub fn clear_cache(&mut self) {
        self.stash.clear();
    }
}
