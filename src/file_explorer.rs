use std::sync::Arc;

use iced::{
    widget::{column, container, scrollable, text, Column},
    Element, Length,
};

use crate::{main_window::Message, parser::DB};

#[derive(Debug, Default)]
pub struct FileExplorer {
    pub db: Arc<DB>,
}

impl FileExplorer {
    pub fn view(&self) -> Element<Message> {
        if self.db.entries.is_empty() {
            return text("Данных нет").into();
        }

        view_db(&self.db)
    }
}

fn view_db(db: &Arc<DB>) -> Element<Message> {
    let mut entries_column = Column::new().spacing(20);
    let mut questions_column = Column::new().spacing(10);
    let mut tips_column = Column::new().spacing(10);

    for entry in db.entries.iter() {
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

    if !db.questions.is_empty() {
        questions_column = questions_column.push(text("Вопросы: ").size(16));
    }
    for (question, answer) in db.questions.iter() {
        questions_column =
            questions_column.push(text(format!("{}: {}", question, answer)).size(16));
    }

    if !db.tips.is_empty() {
        tips_column = tips_column.push(text("Подсказки: ").size(16));
    }
    for (tip, detail) in db.tips.iter() {
        tips_column = tips_column.push(text(format!("{}: {}", tip, detail)).size(16));
    }

    scrollable(column![entries_column, questions_column, tips_column].spacing(24))
        .width(Length::Fill)
        .into()
}
