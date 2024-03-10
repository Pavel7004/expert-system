use std::path::PathBuf;
use std::sync::Arc;

use iced::{
    widget::{button, column, container, row, text, text_editor, vertical_space},
    {executor, theme, Application, Command, Element, Length, Theme},
};
use tokio::io;

use crate::{
    editor::TextEditor,
    file_explorer::FileExplorer,
    logs::Logs,
    parser::{parse_db_from_file, ParserError, DB},
    questions::Questions,
};

pub struct MainWindow {
    db: Arc<DB>,
    file: Option<PathBuf>,

    active_tab: Tabs,

    explorer: FileExplorer,
    logs: Logs,
    editor: TextEditor,
    questions: Questions,
}

#[derive(Debug, Clone)]
pub enum Message {
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
pub enum Tabs {
    #[default]
    Questions,
    Explorer,
    Logs,
    Editor,
}

#[derive(Debug, Clone)]
pub enum Error {
    DialogClosed,
    IO(io::ErrorKind),
    Parse(Arc<String>, (usize, usize)),
    Query(Arc<String>),
}

impl Application for MainWindow {
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
        Theme::Nord
    }

    fn update(&mut self, message: Self::Message) -> Command<Message> {
        match message {
            Message::OpenFile => Command::perform(open_file(), Message::FileOpened),
            Message::FileOpened(result) => match result {
                Ok((path, contents)) => {
                    self.file = Some(path);
                    self.editor.set_content(&contents);

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
                self.editor.perform_action(action);

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
        let tabs = self.tabs();

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
            column![tabs, vertical_space(), file_manager]
                .width(Length::Fixed(240.0))
                .spacing(20),
        );

        let right_pane = match self.active_tab {
            Tabs::Questions => self.questions.view(),
            Tabs::Explorer => self.explorer.view(),
            Tabs::Logs => self.logs.view(),
            Tabs::Editor => self.editor.view(),
        };

        container(row![left_pane, container(right_pane).padding(10)])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

impl MainWindow {
    fn tabs(&self) -> Element<Message> {
        macro_rules! tab {
            ($name: expr, $tab: expr) => {
                button($name)
                    .on_press_maybe((self.active_tab != $tab).then_some(Message::TabChanged($tab)))
                    .width(Length::Fill)
                    .style(theme::Button::Secondary)
            };
        }

        column![
            tab!("Вопросы", Tabs::Questions),
            tab!("Данные", Tabs::Explorer),
            tab!("Редактор", Tabs::Editor),
            tab!("Сообщения", Tabs::Logs),
        ]
        .spacing(5)
        .width(Length::Fill)
        .into()
    }
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
    parse_db_from_file(&contents)
        .map(Arc::new)
        .map_err(|err| match err {
            ParserError::Parse(s, pos) => Error::Parse(Arc::new(s.to_string()), pos),
        })
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
