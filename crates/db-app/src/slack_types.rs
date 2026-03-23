pub struct SlackTeamRow {
    pub id: String,
    pub connection_id: String,
    pub team_id: String,
    pub team_name: String,
    pub created_at: String,
    pub user_id: String,
}

pub struct SlackChannelRow {
    pub id: String,
    pub slack_team_id: String,
    pub channel_id: String,
    pub name: String,
    pub channel_type: String,
    pub is_external: bool,
    pub created_at: String,
    pub user_id: String,
}

pub struct SlackThreadRow {
    pub id: String,
    pub channel_id: String,
    pub thread_ts: String,
    pub started_at: String,
    pub last_message_at: String,
    pub message_count: i32,
    pub created_at: String,
    pub user_id: String,
}

pub struct SlackMessageRow {
    pub id: String,
    pub thread_id: String,
    pub channel_id: String,
    pub alias_id: String,
    pub text: String,
    pub ts: String,
    pub raw_json: String,
    pub created_at: String,
    pub user_id: String,
}

pub struct SlackThreadParticipantRow {
    pub id: String,
    pub thread_id: String,
    pub alias_id: String,
    pub created_at: String,
    pub user_id: String,
}
