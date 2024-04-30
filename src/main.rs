use iced::Application;

use crate::main_window::MainWindow;

mod editor;
mod file_explorer;
mod logs;
mod main_window;
mod parser;
mod questions;

fn main() -> iced::Result {
    MainWindow::run(iced::Settings {
        window: iced::window::Settings {
            resizable: true,
            decorations: true,
            ..iced::window::Settings::default()
        },
        ..iced::Settings::default()
    })
}
