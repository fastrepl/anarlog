use rig::message::Message;

pub(crate) enum Effect {
    Submit {
        prompt: String,
        history: Vec<Message>,
    },
    Exit,
}
