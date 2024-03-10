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
    button, column, combo_box, container, horizontal_space, row, scrollable, text, text_editor,
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
    questions: Questions,
}

#[derive(Debug, Clone)]
enum Message {
    EditorActionPerformed(text_editor::Action),
    TabChanged(Tabs),

    OpenFile,
    FileOpened(Result<(PathBuf, Arc<String>), Error>),
    FileParsed(Result<Arc<DB>, Error>),

    ClearLogs,

    FindAnswer,
    FoundAnswer(Result<Arc<String>, Error>),

    SelectedCategory(Arc<String>),
    SelectedAnswer(Arc<String>, Arc<String>),
}

#[derive(Debug, Clone, Default, PartialEq)]
enum Tabs {
    #[default]
    Questions,
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
                active_tab: Tabs::default(),
                explorer: FileExplorer {
                    db: Arc::clone(&db),
                },
                logs: Logs::default(),
                editor: TextEditor::default(),
                questions: Questions::default(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Экспертная система")
    }

    fn theme(&self) -> Theme {
        Theme::CatppuccinLatte
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
                    self.active_tab = Tabs::Logs;

                    Command::none()
                }
            },
            Message::FileParsed(result) => {
                match result {
                    Ok(db) => {
                        self.db = db.clone();
                        self.explorer.db = db.clone();
                        self.questions.db = db;

                        self.questions.refresh_categories();

                        self.active_tab = Tabs::Questions;
                    }
                    Err(error) => {
                        self.active_tab = Tabs::Logs;

                        self.logs.error(error);
                    }
                }
                Command::none()
            }
            Message::TabChanged(new_tab) => {
                self.active_tab = new_tab;

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
            Message::SelectedAnswer(category, answer) => {
                let (_, answ) = self.questions.answers.get_mut(category.as_ref()).unwrap();

                *answ = Some(answer.to_string());

                Command::none()
            }
            Message::FindAnswer => {
                self.questions.is_searching = true;

                Command::perform(
                    query_db(
                        self.db.clone(),
                        self.questions.selected_category.clone(),
                        self.questions
                            .answers
                            .iter()
                            .filter(|(_, (_, y))| y.is_some())
                            .map(|(x, (_, y))| -> (String, String) {
                                (x.to_string(), y.clone().unwrap())
                            })
                            .collect::<Vec<_>>(),
                    ),
                    Message::FoundAnswer,
                )
            }
            Message::FoundAnswer(res) => {
                match res {
                    Ok(result) => self.questions.result = result,
                    Err(err) => {
                        self.questions.result = Arc::new(String::from("Not found."));

                        self.logs.error(err);
                    }
                };
                self.questions.is_searching = false;

                Command::none()
            }
            Message::SelectedCategory(category) => {
                self.questions.selected_category = Some(category.to_string());

                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let switches = container(
            column![
                button("Вопросы")
                    .on_press_maybe(
                        (self.active_tab != Tabs::Questions)
                            .then_some(Message::TabChanged(Tabs::Questions))
                    )
                    .width(Length::Fill)
                    .style(theme::Button::Secondary),
                button("Данные")
                    .on_press_maybe(
                        (self.active_tab != Tabs::Explorer)
                            .then_some(Message::TabChanged(Tabs::Explorer))
                    )
                    .width(Length::Fill)
                    .style(theme::Button::Secondary),
                button("Редактор")
                    .on_press_maybe(
                        (self.active_tab != Tabs::Editor)
                            .then_some(Message::TabChanged(Tabs::Editor))
                    )
                    .width(Length::Fill)
                    .style(theme::Button::Secondary),
                button("Логи")
                    .on_press_maybe(
                        (self.active_tab != Tabs::Logs).then_some(Message::TabChanged(Tabs::Logs))
                    )
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
            column![switches, vertical_space(), file_manager]
                .width(Length::Fixed(240.0))
                .spacing(20),
        );

        let right_pane = match self.active_tab {
            Tabs::Explorer => self.explorer.view(),
            Tabs::Logs => self.logs.view(),
            Tabs::Editor => self.editor.view(),
            Tabs::Questions => self.questions.view(),
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

#[derive(Debug)]
struct Questions {
    db: Arc<DB>,
    is_searching: bool,

    answers: HashMap<String, (combo_box::State<String>, Option<String>)>,
    result: Arc<String>,

    categories: combo_box::State<String>,
    selected_category: Option<String>,
}

impl Default for Questions {
    fn default() -> Self {
        Self {
            db: Arc::new(DB::default()),
            answers: HashMap::default(),
            result: Arc::new(String::default()),
            categories: combo_box::State::new(vec![]),
            selected_category: None,
            is_searching: false,
        }
    }
}

impl Questions {
    fn view(&self) -> Element<Message> {
        if self.db.entries.is_empty() {
            return container(text("Нет данных")).padding(10).into();
        }

        let find_category = combo_box(
            &self.categories,
            "Выберите категорию для поиска...",
            self.selected_category.as_ref(),
            |cat| Message::SelectedCategory(Arc::new(cat)),
        );

        let find_button =
            button("Найти").on_press_maybe((!self.is_searching).then_some(Message::FindAnswer));

        let questions = self
            .db
            .questions
            .iter()
            .filter(|(category, _)| match self.selected_category {
                Some(ref cat) => category != &cat,
                None => true,
            })
            .fold(Column::new().spacing(10), |column, (category, question)| {
                let (state, selected) = self.answers.get(category).unwrap();
                let category = category.clone();

                column.push(
                    column![
                        text(question),
                        combo_box(state, "Ответ...", selected.as_ref(), move |val| {
                            Message::SelectedAnswer(Arc::new(category.to_string()), Arc::new(val))
                        })
                    ]
                    .spacing(3),
                )
            });

        let mut form = column![find_category, questions, find_button].spacing(10);

        if !self.result.is_empty() {
            form = form.push(text(&self.result));
        }

        container(form).padding(10).into()
    }

    fn refresh_categories(&mut self) {
        self.selected_category = None;

        self.categories = combo_box::State::new(
            self.db
                .categories
                .keys()
                .map(|x| x.to_string())
                .collect::<Vec<_>>(),
        );

        self.db.categories.iter().for_each(|(x, y)| {
            self.answers
                .insert(x.to_string(), (combo_box::State::new(y.clone()), None));
        });
    }
}

#[derive(Debug, Clone)]
enum Error {
    DialogClosed,
    IO(io::ErrorKind),
    Parse(Arc<String>, (usize, usize)),
    Query(Arc<String>),
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

async fn query_db(
    db: Arc<DB>,
    target: Option<String>,
    query: Vec<(String, String)>,
) -> Result<Arc<String>, Error> {
    db.find_value(
        target.as_ref(),
        query.iter().map(|(x, y)| (x, y)).collect::<Vec<_>>(),
    )
    .map(Arc::new)
    .ok_or(Error::Query(Arc::new(format!(
        "Query {:?} didn't find anything, target category {:?}",
        query, target
    ))))
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

    fn find_value(
        &self,
        target_category: Option<&String>,
        query: Vec<(&String, &String)>,
    ) -> Option<String> {
        let mut sub_categories_to_match = Vec::new();

        if let Some(target_cat) = target_category {
            for entry in &self.entries {
                if &entry.category == target_cat {
                    for (cat, val) in &entry.categories {
                        if query
                            .iter()
                            .any(|&(q_cat, q_val)| q_cat == cat && q_val == val)
                        {
                            sub_categories_to_match.push((cat.clone(), val.clone()));
                        }
                    }
                }
            }
        }

        self.entries
            .iter()
            .find(|entry| {
                sub_categories_to_match.iter().all(|(sub_cat, sub_val)| {
                    entry
                        .categories
                        .iter()
                        .any(|(cat, val)| cat == sub_cat && val == sub_val)
                })
            })
            .map(|entry| entry.value.clone())

        // let entries_in_target_category = self
        //     .entries
        //     .iter()
        //     .filter(|entry| target_category.is_some_and(|x| entry.category == *x))
        //     .collect::<Vec<_>>();

        // let mut matching_entries = entries_in_target_category.clone();
        // for (query_cat, query_val) in query {
        //     matching_entries = matching_entries
        //         .into_iter()
        //         .filter(|entry| {
        //             entry
        //                 .categories
        //                 .iter()
        //                 .any(|(cat, val)| cat == query_cat && val == query_val)
        //         })
        //         .collect::<Vec<_>>();
        // }

        // matching_entries
        //     .into_iter()
        //     .next()
        //     .map(|entry| entry.value.clone())
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
