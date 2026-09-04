use std::collections::HashMap;

use anlg_loops::{LoopClient, TransactionalEmail};
use reqwest::{Client, Url};
use serde::Serialize;

use super::INVITATION_TRANSACTIONAL_ID;

#[derive(Clone)]
pub(super) enum EmailDelivery {
    Resend(ResendClient),
    Loops(LoopClient),
}

#[derive(Clone)]
pub(super) struct ResendClient {
    client: Client,
    api_base: Url,
    api_key: String,
    from_email: String,
}

impl ResendClient {
    pub(super) fn new(
        client: Client,
        api_base: Option<Url>,
        api_key: String,
        from_email: String,
    ) -> Self {
        Self {
            client,
            api_base: api_base.unwrap_or_else(|| {
                Url::parse("https://api.resend.com/").expect("Resend API URL must parse")
            }),
            api_key,
            from_email,
        }
    }

    async fn send(
        &self,
        recipient: &str,
        owner_email: &str,
        sender_name: &str,
        subject: String,
        text: String,
        idempotency_key: &str,
    ) -> Result<(), String> {
        let url = self
            .api_base
            .join("emails")
            .map_err(|error| error.to_string())?;
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .header("Idempotency-Key", idempotency_key)
            .json(&ResendEmail {
                from: format!(
                    "{} via Anarlog <{}>",
                    safe_sender_name(sender_name),
                    self.from_email
                ),
                to: recipient.to_string(),
                reply_to: owner_email.to_string(),
                subject,
                text,
            })
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if response.status().is_success() {
            return Ok(());
        }
        Err(format!("Resend returned {}", response.status()))
    }

    async fn send_batch(
        &self,
        recipients: &[String],
        owner_email: &str,
        sender_name: &str,
        subject: String,
        text: String,
        idempotency_key: &str,
    ) -> Result<(), String> {
        let url = self
            .api_base
            .join("emails/batch")
            .map_err(|error| error.to_string())?;
        let from = format!(
            "{} via Anarlog <{}>",
            safe_sender_name(sender_name),
            self.from_email
        );
        let emails = recipients
            .iter()
            .map(|recipient| ResendEmail {
                from: from.clone(),
                to: recipient.clone(),
                reply_to: owner_email.to_string(),
                subject: subject.clone(),
                text: text.clone(),
            })
            .collect::<Vec<_>>();
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .header("Idempotency-Key", idempotency_key)
            .json(&emails)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if response.status().is_success() {
            return Ok(());
        }
        Err(format!("Resend returned {}", response.status()))
    }
}

impl EmailDelivery {
    pub(super) async fn send_invitation(
        &self,
        recipient: &str,
        owner_email: &str,
        sender_name: &str,
        note_title: &str,
        invitation_url: &str,
        invitation_id: &str,
    ) -> Result<(), String> {
        let sender_name = safe_sender_name(sender_name);
        match self {
            Self::Resend(client) => {
                client
                    .send(
                        recipient,
                        owner_email,
                        &sender_name,
                        format!("{sender_name} invited you to {note_title}"),
                        format!(
                            "{} invited you to view \"{}\" in Anarlog.\n\nOpen the meeting notes:\n{}\n\nReply to this email to contact {}.",
                            sender_name,
                            note_title,
                            invitation_url,
                            sender_name
                        ),
                        invitation_id,
                    )
                    .await
            }
            Self::Loops(client) => {
                client
                    .send_transactional(
                        TransactionalEmail {
                            email: recipient.to_string(),
                            transactional_id: INVITATION_TRANSACTIONAL_ID.to_string(),
                            data_variables: HashMap::from([
                                ("senderName".to_string(), sender_name),
                                ("noteTitle".to_string(), note_title.to_string()),
                                ("inviteUrl".to_string(), invitation_url.to_string()),
                            ]),
                        },
                        invitation_id,
                    )
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }
        }
    }

    pub(super) async fn send_workspace_invitation(
        &self,
        recipient: &str,
        owner_email: &str,
        sender_name: &str,
        workspace_name: &str,
        invitation_url: &str,
        invitation_id: &str,
    ) -> Result<(), String> {
        let Self::Resend(client) = self else {
            return Err("Resend is not configured".to_string());
        };
        let sender_name = safe_sender_name(sender_name);
        client
            .send(
                recipient,
                owner_email,
                &sender_name,
                format!("You're invited to join the \"{workspace_name}\" team in Anarlog"),
                format!(
                    "{} invited you to join \"{}\", a team in Anarlog.\n\nAccept the invitation:\n{}\n\nReply to this email to contact {}.",
                    sender_name,
                    workspace_name,
                    invitation_url,
                    sender_name
                ),
                invitation_id,
            )
            .await
    }

    pub(super) async fn send_recap(
        &self,
        recipients: &[String],
        owner_email: &str,
        sender_name: &str,
        note_title: &str,
        note_body: &str,
        idempotency_key: &str,
    ) -> Result<(), String> {
        let Self::Resend(client) = self else {
            return Err("Resend is not configured".to_string());
        };
        client
            .send_batch(
                recipients,
                owner_email,
                sender_name,
                format!("Meeting notes: {note_title}"),
                format!(
                    "{note_title}\n\n{note_body}\n\nSent by {} via Anarlog. Reply to this email to contact them.",
                    safe_sender_name(sender_name)
                ),
                idempotency_key,
            )
            .await
    }
}

#[derive(Serialize)]
struct ResendEmail {
    from: String,
    to: String,
    reply_to: String,
    subject: String,
    text: String,
}

pub(super) fn safe_sender_name(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_control() || matches!(character, '<' | '>' | '"' | '\\') {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let normalized = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized = normalized.chars().take(80).collect::<String>();
    if normalized.is_empty() {
        "An Anarlog user".to_string()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::safe_sender_name;

    #[test]
    fn sanitizes_sender_names_for_email_headers() {
        assert_eq!(safe_sender_name(" Ada <Lovelace>\n"), "Ada Lovelace");
        assert_eq!(safe_sender_name("\r\n"), "An Anarlog user");
    }
}
