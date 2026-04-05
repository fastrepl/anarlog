use hypr_ticket_interface::{CollectionPage, TicketPage};

use crate::error::Error;

fn make_client(api_base_url: &str, access_token: &str) -> Result<hypr_api_client::Client, Error> {
    let auth_value = format!("Bearer {access_token}").parse()?;
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::AUTHORIZATION, auth_value);
    let http = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;
    Ok(hypr_api_client::Client::new_with_client(api_base_url, http))
}

pub async fn linear_list_teams(
    api_base_url: &str,
    access_token: &str,
    connection_id: &str,
    limit: Option<u32>,
    cursor: Option<String>,
) -> Result<CollectionPage, Error> {
    let client = make_client(api_base_url, access_token)?;

    let body = hypr_api_client::types::LinearListTeamsRequest {
        connection_id: connection_id.to_string(),
        limit: limit.map(|l| l as i32),
        cursor,
    };

    let response = client
        .linear_list_teams(&body)
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    Ok(response.into_inner())
}

pub async fn linear_list_tickets(
    api_base_url: &str,
    access_token: &str,
    connection_id: &str,
    team_id: &str,
    query: Option<String>,
    limit: Option<u32>,
    cursor: Option<String>,
) -> Result<TicketPage, Error> {
    let client = make_client(api_base_url, access_token)?;

    let body = hypr_api_client::types::LinearListTicketsRequest {
        connection_id: connection_id.to_string(),
        team_id: team_id.to_string(),
        query,
        limit: limit.map(|l| l as i32),
        cursor,
    };

    let response = client
        .linear_list_tickets(&body)
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    Ok(response.into_inner())
}
