use crate::error::BenchError;

const MAX_ISSUES: usize = 256;

#[derive(Default)]
pub(super) struct Issues {
    messages: Vec<String>,
    omitted: usize,
}

impl Issues {
    pub(super) fn push(&mut self, message: impl Into<String>) {
        if self.messages.len() < MAX_ISSUES {
            self.messages.push(message.into());
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }

    pub(super) fn finish(mut self) -> Result<(), BenchError> {
        if self.messages.is_empty() {
            return Ok(());
        }
        if self.omitted != 0 {
            self.messages.push(format!(
                "{} additional qualification issues omitted",
                self.omitted
            ));
        }
        Err(BenchError::Qualification(self.messages.join("\n")))
    }
}
