//! The last step of posting a notice, once the call's chat input is found: put Anarlog's text in,
//! send only exactly that text, and tell a message that can't have gone out from one that might
//! have. The app is behind `ChatInput`, so these rules are tested without a call.

use super::{Outcome, matching::is_empty_draft};

/// The chat input of one call, as the poster drives it.
pub(crate) trait ChatInput {
    fn value(&self) -> String;
    fn placeholder(&self) -> String;
    /// Sets the text directly, where the app takes a value; false where it doesn't.
    fn set_value(&self, text: &str) -> bool;
    fn focus(&self);
    /// Keys reach this input: its app is at the front and this input has focus.
    fn can_type(&self) -> bool;
    fn paste(&self, text: &str) -> bool;
    /// Select All, then Delete.
    fn erase(&self) -> bool;
    /// The one send action reaches this input's chat now. For Return that is `can_type`.
    fn can_send(&self) -> bool;
    /// Presses Return. Called at most once per delivery.
    fn send(&self) -> bool;
    /// Polls `ready` until it holds or one step's time passes.
    fn wait_until(&self, ready: &mut dyn FnMut() -> bool) -> bool;
}

/// Puts `message` in the empty input and sends it. Only `Sent` and `Unconfirmed` follow the send
/// action; every other outcome means Anarlog sent nothing, and its text was taken back out.
pub(crate) fn deliver(
    input: &impl ChatInput,
    message: &str,
    cancelled: &dyn Fn() -> bool,
) -> Outcome {
    let message = message.trim();
    // The person may have started typing while the app came to the front.
    if !is_empty_draft(&input.value(), &input.placeholder()) {
        return Outcome::DraftInProgress;
    }
    if cancelled() {
        return Outcome::Cancelled;
    }
    let outcome = insert_and_send(input, message, cancelled);
    // Anarlog's text must not stay behind in the person's chat box for them to send by accident.
    if outcome != Outcome::Sent && input.value().trim() == message {
        take_back(input, message);
    }
    outcome
}

fn insert_and_send(input: &impl ChatInput, message: &str, cancelled: &dyn Fn() -> bool) -> Outcome {
    input.focus();
    // A native text view takes the value directly; a web view needs a paste.
    let set = input.set_value(message) && input.wait_until(&mut || input.value().trim() == message);
    if !set {
        if cancelled() {
            return Outcome::Cancelled;
        }
        if !input.can_type() || !input.paste(message) {
            return Outcome::NotSent;
        }
        input.wait_until(&mut || input.value().trim() == message);
    }
    let value = input.value();
    if value.trim() != message {
        // Typed into while Anarlog waited: the text is the person's now.
        return if is_empty_draft(&value, &input.placeholder()) {
            Outcome::NotSent
        } else {
            Outcome::DraftInProgress
        };
    }
    // A recording that stopped meanwhile takes its notice back, up to the moment it is sent.
    if cancelled() {
        return Outcome::Cancelled;
    }
    if !input.can_send() || !input.send() {
        return Outcome::NotSent;
    }
    // From here the message may have gone out, so it is never posted again, whatever this says.
    if input.wait_until(&mut || !input.value().contains(message)) {
        Outcome::Sent
    } else {
        Outcome::Unconfirmed
    }
}

/// Empties the input while it still holds exactly `message`: directly where the app takes a value,
/// else with Select All and Delete, only while keys reach the input.
fn take_back(input: &impl ChatInput, message: &str) {
    if input.set_value("") && input.wait_until(&mut || input.value().trim() != message) {
        return;
    }
    if input.value().trim() == message && input.can_type() {
        input.erase();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;

    const NOTICE: &str = "I'm using Anarlog to record and transcribe this meeting.";

    type Change = Box<dyn FnMut(&mut String)>;

    /// A chat input that takes text by paste only, as the web views do, and clears on send.
    struct Fake {
        value: RefCell<String>,
        takes_value: bool,
        focused: Cell<bool>,
        clears_on_send: bool,
        /// The paste shows only once Anarlog waits on it.
        slow_paste: bool,
        pending_paste: RefCell<Option<String>>,
        sends: Cell<usize>,
        /// What the person or the app does to the text while Anarlog waits on it.
        meanwhile: RefCell<Option<Change>>,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                value: RefCell::new(String::new()),
                takes_value: false,
                focused: Cell::new(true),
                clears_on_send: true,
                slow_paste: false,
                pending_paste: RefCell::new(None),
                sends: Cell::new(0),
                meanwhile: RefCell::new(None),
            }
        }

        fn meanwhile(self, change: impl FnMut(&mut String) + 'static) -> Self {
            *self.meanwhile.borrow_mut() = Some(Box::new(change));
            self
        }
    }

    impl ChatInput for Fake {
        fn value(&self) -> String {
            self.value.borrow().clone()
        }
        fn placeholder(&self) -> String {
            "Type a message".into()
        }
        fn set_value(&self, text: &str) -> bool {
            if self.takes_value {
                *self.value.borrow_mut() = text.into();
            }
            self.takes_value
        }
        fn focus(&self) {}
        fn can_type(&self) -> bool {
            self.focused.get()
        }
        fn paste(&self, text: &str) -> bool {
            if self.slow_paste {
                *self.pending_paste.borrow_mut() = Some(text.into());
            } else {
                self.value.borrow_mut().push_str(text);
            }
            true
        }
        fn erase(&self) -> bool {
            self.value.borrow_mut().clear();
            true
        }
        fn can_send(&self) -> bool {
            self.focused.get()
        }
        fn send(&self) -> bool {
            self.sends.set(self.sends.get() + 1);
            if self.clears_on_send {
                self.value.borrow_mut().clear();
            }
            true
        }
        fn wait_until(&self, ready: &mut dyn FnMut() -> bool) -> bool {
            for _ in 0..3 {
                if ready() {
                    return true;
                }
                if let Some(text) = self.pending_paste.borrow_mut().take() {
                    self.value.borrow_mut().push_str(&text);
                }
                if let Some(change) = self.meanwhile.borrow_mut().as_mut() {
                    change(&mut self.value.borrow_mut());
                }
            }
            ready()
        }
    }

    fn not_cancelled() -> bool {
        false
    }

    #[test]
    fn sends_the_notice_with_one_return() {
        let input = Fake::new();
        assert_eq!(deliver(&input, NOTICE, &not_cancelled), Outcome::Sent);
        assert_eq!(input.sends.get(), 1);
    }

    #[test]
    fn a_return_the_app_never_confirms_is_unconfirmed_and_pressed_once() {
        let input = Fake {
            clears_on_send: false,
            ..Fake::new()
        };
        assert_eq!(
            deliver(&input, NOTICE, &not_cancelled),
            Outcome::Unconfirmed
        );
        assert_eq!(input.sends.get(), 1);
        // Taken back out, so the person can't send it again by accident.
        assert_eq!(input.value(), "");
    }

    #[test]
    fn leaves_a_draft_typed_while_the_app_came_to_the_front() {
        let input = Fake::new();
        *input.value.borrow_mut() = "one sec".into();
        assert_eq!(
            deliver(&input, NOTICE, &not_cancelled),
            Outcome::DraftInProgress
        );
        assert_eq!(input.value(), "one sec");
        assert_eq!(input.sends.get(), 0);
    }

    #[test]
    fn leaves_the_text_to_the_person_who_typed_while_the_paste_landed() {
        let input = Fake {
            slow_paste: true,
            ..Fake::new()
        }
        .meanwhile(|value| {
            if value.as_str() == NOTICE {
                value.push_str(" Also, wait");
            }
        });
        assert_eq!(
            deliver(&input, NOTICE, &not_cancelled),
            Outcome::DraftInProgress
        );
        assert_eq!(input.value(), format!("{NOTICE} Also, wait"));
        assert_eq!(input.sends.get(), 0);
    }

    #[test]
    fn a_stop_before_return_takes_the_notice_back() {
        let input = Fake::new();
        let checks = Cell::new(0);
        // Live for the checks before the paste, stopped by the one before Return.
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() > 2
        };
        assert_eq!(deliver(&input, NOTICE, &cancelled), Outcome::Cancelled);
        assert_eq!(input.sends.get(), 0);
        assert_eq!(input.value(), "");
    }

    #[test]
    fn a_stop_after_return_cannot_turn_it_into_cancelled() {
        let input = Fake {
            takes_value: true,
            clears_on_send: false,
            ..Fake::new()
        };
        // Cancelled from the moment Return was pressed: too late to take back.
        let cancelled = || input.sends.get() > 0;
        assert_eq!(deliver(&input, NOTICE, &cancelled), Outcome::Unconfirmed);
        assert_eq!(input.sends.get(), 1);
    }

    #[test]
    fn presses_nothing_once_the_person_switched_away() {
        let input = Fake {
            takes_value: true,
            ..Fake::new()
        };
        input.focused.set(false);
        assert_eq!(deliver(&input, NOTICE, &not_cancelled), Outcome::NotSent);
        assert_eq!(input.sends.get(), 0);
        assert_eq!(input.value(), "");
    }
}
