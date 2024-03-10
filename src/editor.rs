use iced::{
    widget::{column, horizontal_space, row, text, text_editor},
    Element, Length,
};

use crate::main_window::Message;

#[derive(Debug)]
pub struct TextEditor {
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
    pub fn view(&self) -> Element<Message> {
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

    pub fn set_content(&mut self, cont: &str) {
        self.content = text_editor::Content::with_text(cont);
    }

    pub fn perform_action(&mut self, action: text_editor::Action) {
        self.content.perform(action);
    }
}
