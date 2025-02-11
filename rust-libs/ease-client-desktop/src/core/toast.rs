use std::collections::VecDeque;

pub struct Toast {
    messages: VecDeque<String>,
}

impl Toast {
    pub fn show(&mut self, msg: String) {
        self.messages.push_back(msg);
    }

    pub fn pop(&mut self) {
        self.messages.pop_front();
    }

    pub fn get_messages(&self) -> &VecDeque<String> {
        return &self.messages;
    }
}
