use std::{collections::HashMap, sync::Arc};

use iced::{
    widget::{button, column, combo_box, text, Column},
    Element,
};

use crate::{main_window::Message, parser::DB};

#[derive(Debug)]
pub struct Questions {
    pub db: Arc<DB>,
    pub is_searching: bool,

    pub answers: HashMap<String, (combo_box::State<String>, Option<String>)>,
    pub result: Arc<String>,

    pub selected_category: Option<String>,

    categories: combo_box::State<String>,
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
    pub fn view(&self) -> Element<Message> {
        if self.db.entries.is_empty() {
            return text("Нет данных").into();
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

        form.into()
    }

    pub fn refresh_categories(&mut self) {
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
