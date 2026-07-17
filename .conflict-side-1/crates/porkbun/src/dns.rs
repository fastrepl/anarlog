use crate::client::{PorkbunClient, parse_response};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DnsRecord {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub content: String,
    pub ttl: String,
    pub prio: Option<String>,
    pub notes: Option<String>,
}

#[derive(serde::Serialize)]
struct AuthBody {
    secretapikey: String,
    apikey: String,
}

#[derive(serde::Serialize)]
struct CreateRecordBody {
    secretapikey: String,
    apikey: String,
    r#type: String,
    content: String,
    name: Option<String>,
    ttl: Option<String>,
    prio: Option<String>,
}

#[derive(serde::Serialize)]
struct EditRecordBody {
    secretapikey: String,
    apikey: String,
    r#type: String,
    content: String,
    name: Option<String>,
    ttl: Option<String>,
    prio: Option<String>,
}

#[derive(serde::Deserialize)]
struct StatusResponse {
    #[allow(dead_code)]
    status: String,
}

#[derive(serde::Deserialize)]
struct CreateRecordResponse {
    #[allow(dead_code)]
    status: String,
    pub id: i64,
}

#[derive(serde::Deserialize)]
struct RetrieveRecordsResponse {
    #[allow(dead_code)]
    status: String,
    pub records: Vec<DnsRecord>,
}

impl PorkbunClient {
    fn auth_body(&self) -> AuthBody {
        AuthBody {
            secretapikey: self.secret_api_key.clone(),
            apikey: self.api_key.clone(),
        }
    }

    pub async fn dns_create(
        &self,
        domain: &str,
        record_type: &str,
        content: &str,
        name: Option<&str>,
        ttl: Option<&str>,
        prio: Option<&str>,
    ) -> Result<i64, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("/api/json/v3/dns/create/{}", domain));

        let body = CreateRecordBody {
            secretapikey: self.secret_api_key.clone(),
            apikey: self.api_key.clone(),
            r#type: record_type.to_string(),
            content: content.to_string(),
            name: name.map(|s| s.to_string()),
            ttl: ttl.map(|s| s.to_string()),
            prio: prio.map(|s| s.to_string()),
        };

        let response = self.client.post(url).json(&body).send().await?;
        let parsed: CreateRecordResponse = parse_response(response).await?;
        Ok(parsed.id)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn dns_edit(
        &self,
        domain: &str,
        id: &str,
        record_type: &str,
        content: &str,
        name: Option<&str>,
        ttl: Option<&str>,
        prio: Option<&str>,
    ) -> Result<(), crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("/api/json/v3/dns/edit/{}/{}", domain, id));

        let body = EditRecordBody {
            secretapikey: self.secret_api_key.clone(),
            apikey: self.api_key.clone(),
            r#type: record_type.to_string(),
            content: content.to_string(),
            name: name.map(|s| s.to_string()),
            ttl: ttl.map(|s| s.to_string()),
            prio: prio.map(|s| s.to_string()),
        };

        let response = self.client.post(url).json(&body).send().await?;
        let _: StatusResponse = parse_response(response).await?;
        Ok(())
    }

    pub async fn dns_delete(&self, domain: &str, id: &str) -> Result<(), crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("/api/json/v3/dns/delete/{}/{}", domain, id));

        let response = self.client.post(url).json(&self.auth_body()).send().await?;
        let _: StatusResponse = parse_response(response).await?;
        Ok(())
    }

    pub async fn dns_retrieve(
        &self,
        domain: &str,
        id: Option<&str>,
    ) -> Result<Vec<DnsRecord>, crate::Error> {
        let mut url = self.api_base.clone();
        match id {
            Some(id) => url.set_path(&format!("/api/json/v3/dns/retrieve/{}/{}", domain, id)),
            None => url.set_path(&format!("/api/json/v3/dns/retrieve/{}", domain)),
        }

        let response = self.client.post(url).json(&self.auth_body()).send().await?;
        let parsed: RetrieveRecordsResponse = parse_response(response).await?;
        Ok(parsed.records)
    }

    pub async fn dns_retrieve_by_type(
        &self,
        domain: &str,
        record_type: &str,
        subdomain: &str,
    ) -> Result<Vec<DnsRecord>, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!(
            "/api/json/v3/dns/retrieveByNameType/{}/{}/{}",
            domain, record_type, subdomain
        ));

        let response = self.client.post(url).json(&self.auth_body()).send().await?;
        let parsed: RetrieveRecordsResponse = parse_response(response).await?;
        Ok(parsed.records)
    }

    pub async fn ping(&self) -> Result<String, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/api/json/v3/ping");

        let response = self.client.post(url).json(&self.auth_body()).send().await?;
        let parsed: PingResponse = parse_response(response).await?;
        Ok(parsed.your_ip)
    }
}

#[derive(serde::Deserialize)]
struct PingResponse {
    #[allow(dead_code)]
    status: String,
    #[serde(rename = "yourIp")]
    your_ip: String,
}
