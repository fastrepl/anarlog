use hypr_http::HttpClient;

use crate::error::Error;
use crate::types::{
    Attachment, GetMessageRequest, GetThreadRequest, HistoryType, ListHistoryRequest,
    ListHistoryResponse, ListLabelsResponse, ListMessagesRequest, ListMessagesResponse,
    ListThreadsRequest, ListThreadsResponse, Message, MessageFormat, Profile, Thread,
};

pub struct GoogleMailClient<C> {
    http: C,
}

impl<C: HttpClient> GoogleMailClient<C> {
    pub fn new(http: C) -> Self {
        Self { http }
    }

    pub async fn list_labels(&self) -> Result<ListLabelsResponse, Error> {
        let bytes = self
            .http
            .get("/gmail/v1/users/me/labels")
            .await
            .map_err(Error::Http)?;
        let response: ListLabelsResponse = serde_json::from_slice(&bytes)?;
        Ok(response)
    }

    pub async fn list_messages(
        &self,
        req: ListMessagesRequest,
    ) -> Result<ListMessagesResponse, Error> {
        let mut query_parts: Vec<String> = Vec::new();

        if let Some(ref q) = req.q {
            query_parts.push(format!("q={}", urlencoding::encode(q)));
        }
        if let Some(max_results) = req.max_results {
            query_parts.push(format!("maxResults={max_results}"));
        }
        if let Some(ref page_token) = req.page_token {
            query_parts.push(format!("pageToken={}", urlencoding::encode(page_token)));
        }
        if let Some(ref label_ids) = req.label_ids {
            for id in label_ids {
                query_parts.push(format!("labelIds={}", urlencoding::encode(id)));
            }
        }
        if let Some(include_spam_trash) = req.include_spam_trash {
            query_parts.push(format!("includeSpamTrash={include_spam_trash}"));
        }

        let path = if query_parts.is_empty() {
            "/gmail/v1/users/me/messages".to_string()
        } else {
            format!("/gmail/v1/users/me/messages?{}", query_parts.join("&"))
        };

        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        let response: ListMessagesResponse = serde_json::from_slice(&bytes)?;
        Ok(response)
    }

    pub async fn get_message(&self, req: GetMessageRequest) -> Result<Message, Error> {
        let id = &req.id;
        let mut query_parts: Vec<String> = Vec::new();

        if let Some(ref format) = req.format {
            let value = match format {
                MessageFormat::Full => "full",
                MessageFormat::Metadata => "metadata",
                MessageFormat::Minimal => "minimal",
                MessageFormat::Raw => "raw",
            };
            query_parts.push(format!("format={value}"));
        }
        if let Some(ref headers) = req.metadata_headers {
            for h in headers {
                query_parts.push(format!("metadataHeaders={}", urlencoding::encode(h)));
            }
        }

        let path = if query_parts.is_empty() {
            format!("/gmail/v1/users/me/messages/{id}")
        } else {
            format!("/gmail/v1/users/me/messages/{id}?{}", query_parts.join("&"))
        };

        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        let message: Message = serde_json::from_slice(&bytes)?;
        Ok(message)
    }

    pub async fn get_attachment(
        &self,
        message_id: &str,
        attachment_id: &str,
    ) -> Result<Attachment, Error> {
        let path = format!("/gmail/v1/users/me/messages/{message_id}/attachments/{attachment_id}");
        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        let attachment: Attachment = serde_json::from_slice(&bytes)?;
        Ok(attachment)
    }

    pub async fn get_profile(&self) -> Result<Profile, Error> {
        let bytes = self
            .http
            .get("/gmail/v1/users/me/profile")
            .await
            .map_err(Error::Http)?;
        let profile: Profile = serde_json::from_slice(&bytes)?;
        Ok(profile)
    }

    pub async fn list_threads(
        &self,
        req: ListThreadsRequest,
    ) -> Result<ListThreadsResponse, Error> {
        let mut query_parts: Vec<String> = Vec::new();

        if let Some(ref q) = req.q {
            query_parts.push(format!("q={}", urlencoding::encode(q)));
        }
        if let Some(max_results) = req.max_results {
            query_parts.push(format!("maxResults={max_results}"));
        }
        if let Some(ref page_token) = req.page_token {
            query_parts.push(format!("pageToken={}", urlencoding::encode(page_token)));
        }
        if let Some(ref label_ids) = req.label_ids {
            for id in label_ids {
                query_parts.push(format!("labelIds={}", urlencoding::encode(id)));
            }
        }
        if let Some(include_spam_trash) = req.include_spam_trash {
            query_parts.push(format!("includeSpamTrash={include_spam_trash}"));
        }

        let path = if query_parts.is_empty() {
            "/gmail/v1/users/me/threads".to_string()
        } else {
            format!("/gmail/v1/users/me/threads?{}", query_parts.join("&"))
        };

        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        let response: ListThreadsResponse = serde_json::from_slice(&bytes)?;
        Ok(response)
    }

    pub async fn get_thread(&self, req: GetThreadRequest) -> Result<Thread, Error> {
        let id = &req.id;
        let mut query_parts: Vec<String> = Vec::new();

        if let Some(ref format) = req.format {
            let value = match format {
                MessageFormat::Full => "full",
                MessageFormat::Metadata => "metadata",
                MessageFormat::Minimal => "minimal",
                MessageFormat::Raw => "raw",
            };
            query_parts.push(format!("format={value}"));
        }
        if let Some(ref headers) = req.metadata_headers {
            for h in headers {
                query_parts.push(format!("metadataHeaders={}", urlencoding::encode(h)));
            }
        }

        let path = if query_parts.is_empty() {
            format!("/gmail/v1/users/me/threads/{id}")
        } else {
            format!("/gmail/v1/users/me/threads/{id}?{}", query_parts.join("&"))
        };

        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        let thread: Thread = serde_json::from_slice(&bytes)?;
        Ok(thread)
    }

    pub async fn list_history(
        &self,
        req: ListHistoryRequest,
    ) -> Result<ListHistoryResponse, Error> {
        let mut query_parts: Vec<String> = vec![format!(
            "startHistoryId={}",
            urlencoding::encode(&req.start_history_id)
        )];

        if let Some(max_results) = req.max_results {
            query_parts.push(format!("maxResults={max_results}"));
        }
        if let Some(ref page_token) = req.page_token {
            query_parts.push(format!("pageToken={}", urlencoding::encode(page_token)));
        }
        if let Some(ref label_id) = req.label_id {
            query_parts.push(format!("labelId={}", urlencoding::encode(label_id)));
        }
        if let Some(ref history_types) = req.history_types {
            for ht in history_types {
                let value = match ht {
                    HistoryType::MessageAdded => "messageAdded",
                    HistoryType::MessageDeleted => "messageDeleted",
                    HistoryType::LabelAdded => "labelAdded",
                    HistoryType::LabelRemoved => "labelRemoved",
                };
                query_parts.push(format!("historyTypes={value}"));
            }
        }

        let path = format!("/gmail/v1/users/me/history?{}", query_parts.join("&"));

        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        let response: ListHistoryResponse = serde_json::from_slice(&bytes)?;
        Ok(response)
    }
}
