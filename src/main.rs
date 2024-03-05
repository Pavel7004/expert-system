use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Local;

use pest::error::LineColLocation;
use pest::{iterators::Pairs, Parser};
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "syn.pest"]
pub struct LangParser;

#[derive(Default, Debug)]
struct DB {
    entries: Vec<Entry>,
    categories: HashMap<String, Vec<String>>,
    questions: HashMap<String, String>,
    changes: HashMap<String, String>,
    tips: HashMap<String, String>,
}

#[derive(Default, Debug)]
struct Entry {
    value: String,
    category: String,
    categories: Vec<(String, String)>,
}

use iced::widget::{
    button, column, container, horizontal_space, row, scrollable, text, text_editor,
    vertical_space, Column,
};
use iced::{executor, theme, Application, Command, Element, Length, Theme};
use tokio::io;

struct App {
    db: Arc<DB>,
    file: Option<PathBuf>,

    active_tab: Tabs,

    explorer: FileExplorer,
    logs: Logs,
    editor: TextEditor,
}

#[derive(Debug, Clone)]
enum Message {
    EditorActionPerformed(text_editor::Action),
    TabChanged(Tabs),

    OpenFile,
    FileOpened(Result<(PathBuf, Arc<String>), Error>),
    FileParsed(Result<Arc<DB>, Error>),

    ClearLogs,
}

#[derive(Debug, Clone, Default)]
enum Tabs {
    #[default]
    Explorer,
    Logs,
    Editor,
}

impl Application for App {
    type Message = Message;
    type Theme = Theme;
    type Flags = ();
    type Executor = executor::Default;

    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        let db = Arc::new(DB::default());
        (
            Self {
                db: Arc::clone(&db),
                file: None,
                active_tab_id: Tabs::default(),
                explorer: FileExplorer {
                    db: Arc::clone(&db),
                },
                logs: Logs::default(),
                editor: TextEditor::default(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Экспертная система")
    }

    fn theme(&self) -> Theme {
        Theme::Nord
    }

    fn update(&mut self, message: Self::Message) -> Command<Message> {
        match message {
            Message::OpenFile => Command::perform(open_file(), Message::FileOpened),
            Message::FileOpened(result) => match result {
                Ok((path, contents)) => {
                    self.file = Some(path);
                    self.editor.content = text_editor::Content::with_text(&contents);

                    Command::perform(parse_file(contents), Message::FileParsed)
                }
                Err(error) => {
                    self.logs.error(error);
                    self.active_tab_id = Tabs::Logs;

                    Command::none()
                }
            },
            Message::FileParsed(result) => {
                match result {
                    Ok(db) => {
                        self.db = db.clone();
                        self.explorer.db = db;
                        self.active_tab_id = Tabs::Explorer;
                    }
                    Err(error) => {
                        self.active_tab_id = Tabs::Logs;

                        self.logs.error(error);
                    }
                }
                Command::none()
            }
            Message::TabChanged(new_tab) => {
                self.active_tab_id = new_tab;

                Command::none()
            }
            Message::EditorActionPerformed(action) => {
                self.editor.content.perform(action);

                Command::none()
            }
            Message::ClearLogs => {
                self.logs.clear_cache();

                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let switches = container(
            column![
                button("Данные")
                    .on_press(Message::TabChanged(Tabs::Explorer))
                    .width(Length::Fill)
                    .style(theme::Button::Secondary),
                button("Редактор")
                    .on_press(Message::TabChanged(Tabs::Editor))
                    .width(Length::Fill)
                    .style(theme::Button::Secondary),
                button("Логи")
                    .on_press(Message::TabChanged(Tabs::Logs))
                    .width(Length::Fill)
                    .style(theme::Button::Secondary),
            ]
            .spacing(5)
            .width(Length::Fill),
        );

        let file_indicator = text(
            self.file
                .as_ref()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or("Файл не выбран"),
        );

        let file_manager = container(
            column![
                button("Открыть файл...")
                    .on_press(Message::OpenFile)
                    .width(Length::Fill)
                    .style(theme::Button::Primary),
                file_indicator,
            ]
            .spacing(8)
            .padding(8),
        )
        .style(theme::Container::Box);

        let left_pane = container(
            column![file_manager, vertical_space(), switches]
                .width(Length::Fixed(240.0))
                .spacing(20),
        );

        let right_pane = match self.active_tab {
            Tabs::Explorer => self.explorer.view(),
            Tabs::Logs => self.logs.view(),
            Tabs::Editor => self.editor.view(),
        };

        container(row![left_pane, right_pane].spacing(5))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

impl DB {
    pub fn view(&self) -> Element<Message> {
        let mut entries_column = Column::new().spacing(20);
        let mut questions_column = Column::new().spacing(10);
        let mut tips_column = Column::new().spacing(10);

        for entry in &self.entries {
            let entry_container = column![
                text(format!("{}: {}", entry.category, entry.value)).size(18),
                entry
                    .categories
                    .iter()
                    .fold(Column::new().spacing(3), |col, (cat, val)| {
                        col.push(
                            text(format!("{}: {}", cat, val))
                                .size(14)
                                .width(Length::Fill),
                        )
                    }),
            ]
            .spacing(10);

            entries_column = entries_column.push(container(entry_container).width(Length::Fill));
        }

        if !self.questions.is_empty() {
            questions_column = questions_column.push(text("Вопросы: ").size(16));
        }
        for (question, answer) in &self.questions {
            questions_column =
                questions_column.push(text(format!("{}: {}", question, answer)).size(16));
        }

        if !self.tips.is_empty() {
            tips_column = tips_column.push(text("Подсказки: ").size(16));
        }
        for (tip, detail) in &self.tips {
            tips_column = tips_column.push(text(format!("{}: {}", tip, detail)).size(16));
        }

        scrollable(
            column![entries_column, questions_column, tips_column]
                .spacing(24)
                .padding(10),
        )
        .width(Length::Fill)
        .into()
    }
}

#[derive(Debug, Default)]
struct FileExplorer {
    db: Arc<DB>,
}

impl FileExplorer {
    fn view(&self) -> Element<Message> {
        if self.db.entries.is_empty() {
            return container(text("Данных нет").size(16)).padding(10).into();
        }

        self.db.view()
    }
}

#[derive(Debug, Clone)]
struct LogEntry {
    severity: LogSeverity,
    timestamp: String,
    message: String,
}

#[derive(Debug, Clone)]
enum LogSeverity {
    Info,
    //Warning,
    Error,
}

#[derive(Debug, Default)]
struct Logs {
    stash: Vec<LogEntry>,
}

impl Logs {
    fn view(&self) -> Element<Message> {
        if self.stash.is_empty() {
            return container(text("Сообщений нет")).padding(10).into();
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

        container(
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
            .padding(5),
        )
        .into()
    }

    fn error(&mut self, err: Error) {
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
            }
        });
    }

    fn clear_cache(&mut self) {
        self.stash.clear();
    }
}

#[derive(Debug)]
struct TextEditor {
    content: text_editor::Content,
}

impl Default for TextEditor {
    fn default() -> Self {
        Self {
            content: text_editor::Content::new(),
        }
    }
}

impl TextEditor {
    fn view(&self) -> Element<Message> {
        column![
            text_editor(&self.content)
                .height(Length::Fill)
                .on_action(Message::EditorActionPerformed),
            row![
                horizontal_space(),
                text({
                    let (line, column) = self.content.cursor_position();
                    format!("{}:{}", line + 1, column + 1)
                })
            ]
        ]
        .into()
    }
}

#[derive(Debug, Clone)]
enum Error {
    DialogClosed,
    IO(io::ErrorKind),
    Parse(Arc<String>, (usize, usize)),
}

async fn open_file() -> Result<(PathBuf, Arc<String>), Error> {
    let picked_file = rfd::AsyncFileDialog::new()
        .set_title("Открыть базу знаний...")
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)?;

    load_file(picked_file.path().to_owned()).await
}

async fn load_file(path: PathBuf) -> Result<(PathBuf, Arc<String>), Error> {
    let contents = tokio::fs::read_to_string(&path)
        .await
        .map(Arc::new)
        .map_err(|err| err.kind())
        .map_err(Error::IO)?;

    Ok((path, contents))
}

async fn parse_file(contents: Arc<String>) -> Result<Arc<DB>, Error> {
    parse_db_from_file(&contents).map(Arc::new)
}

fn main() -> iced::Result {
    App::run(iced::Settings {
        window: iced::window::Settings {
            resizable: false,
            decorations: true,
            ..iced::window::Settings::default()
        },
        ..iced::Settings::default()
    })
}

fn parse_db_from_file(contents: &str) -> Result<DB, Error> {
    let file = LangParser::parse(Rule::file, contents)
        .map_err(|err| {
            let pos = match err.line_col {
                LineColLocation::Pos((x, y)) => (x, y),
                LineColLocation::Span((start_x, start_y), _) => (start_x, start_y),
            };
            Error::Parse(Arc::new(err.to_string()), pos)
        })?
        .next()
        .unwrap();

    let mut db = DB::new();
    for data in file.into_inner() {
        match data.as_rule() {
            Rule::entry => parse_entry(&mut data.into_inner(), &mut db),
            Rule::advice => parse_advice(&mut data.into_inner(), &mut db.questions),
            Rule::change => parse_change(&mut data.into_inner(), &mut db.changes),
            Rule::tip => parse_tip(&mut data.into_inner(), &mut db.tips),
            Rule::EOI => break,
            _ => unreachable!(),
        }
    }

    Ok(db)
}

impl DB {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            categories: HashMap::new(),
            questions: HashMap::new(),
            changes: HashMap::new(),
            tips: HashMap::new(),
        }
    }

    fn add_category(&mut self, category: &str, value: &str) {
        if let Some(values) = self.categories.get_mut(category) {
            if !values.iter().any(|x| *x == value) {
                values.push(value.to_string());
            }
            return;
        }

        self.categories
            .insert(category.to_string(), vec![value.to_string()]);
    }
}

fn parse_entry(entry: &mut Pairs<'_, Rule>, db: &mut DB) {
    let _number = entry.next().unwrap().as_str().parse::<i32>().unwrap();

    let mut pairs = Vec::<(String, String)>::new();
    entry.next().unwrap().into_inner().for_each(|x| {
        let mut pair = x.into_inner();
        let category = pair.next().unwrap().as_str().to_string();
        let value = pair.next().unwrap().as_str().to_string();

        db.add_category(&category, &value);
        pairs.push((category, value));
    });

    let mut pair = entry.next().unwrap().into_inner();
    let category = pair.next().unwrap().as_str().to_string();
    let value = pair.next().unwrap().as_str().to_string();

    db.add_category(&category, &value);

    db.entries.push(Entry {
        value,
        category,
        categories: pairs,
    });
}

fn parse_advice(advice: &mut Pairs<'_, Rule>, questions: &mut HashMap<String, String>) {
    let category = advice.next().unwrap().as_str().to_string();
    let question = advice.next().unwrap().as_str().to_string();

    questions.insert(category, question);
}

fn parse_change(change: &mut Pairs<'_, Rule>, changes: &mut HashMap<String, String>) {
    let category = change.next().unwrap().as_str().to_string();
    let text = change.next().unwrap().as_str().to_string();

    changes.insert(category, text);
}

fn parse_tip(change: &mut Pairs<'_, Rule>, tips: &mut HashMap<String, String>) {
    let category = change.next().unwrap().as_str().to_string();
    let text = change.next().unwrap().as_str().to_string();

    tips.insert(category, text);
}
