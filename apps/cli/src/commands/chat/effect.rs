use rig::message::Message;

pub(crate) enum Effect {
    Submit {
        prompt: String,
        history: Vec<Message>,
    },
    GenerateTitle {
        prompt: String,
        response: String,
    },
    Exit,
}
