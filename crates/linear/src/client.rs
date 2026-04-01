use hypr_http::HttpClient;

use crate::error::Error;
use crate::types::{
    Connection, GraphQLResponse, Issue, IssuesData, ListIssuesRequest, ListTeamsRequest, Team,
    TeamIssuesData, TeamsData,
};

pub struct LinearClient<C> {
    http: C,
}

impl<C: HttpClient> LinearClient<C> {
    pub fn new(http: C) -> Self {
        Self { http }
    }

    async fn query<T: serde::de::DeserializeOwned>(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<T, Error> {
        let body = serde_json::json!({
            "query": query,
            "variables": variables,
        });
        let bytes = serde_json::to_vec(&body)?;

        let response_bytes = self
            .http
            .post("/graphql", bytes, "application/json")
            .await
            .map_err(Error::Http)?;

        let response: GraphQLResponse<T> = serde_json::from_slice(&response_bytes)?;

        if let Some(errors) = response.errors {
            if !errors.is_empty() {
                let messages: Vec<_> = errors.iter().map(|e| e.message.as_str()).collect();
                return Err(Error::GraphQL(messages.join("; ")));
            }
        }

        response
            .data
            .ok_or_else(|| Error::GraphQL("No data in response".to_string()))
    }

    pub async fn list_teams(&self, req: ListTeamsRequest) -> Result<Connection<Team>, Error> {
        let first = req.first.unwrap_or(50);
        let mut variables = serde_json::json!({ "first": first });
        if let Some(ref after) = req.after {
            variables["after"] = serde_json::json!(after);
        }

        let query = r#"
            query ListTeams($first: Int!, $after: String) {
                teams(first: $first, after: $after) {
                    nodes {
                        id
                        name
                        key
                        description
                    }
                    pageInfo {
                        hasNextPage
                        endCursor
                    }
                }
            }
        "#;

        let data: TeamsData = self.query(query, variables).await?;
        Ok(data.teams)
    }

    pub async fn list_issues(&self, req: ListIssuesRequest) -> Result<Connection<Issue>, Error> {
        let first = req.first.unwrap_or(50);

        if let Some(ref team_id) = req.team_id {
            let mut variables = serde_json::json!({
                "teamId": team_id,
                "first": first,
            });
            if let Some(ref after) = req.after {
                variables["after"] = serde_json::json!(after);
            }
            if let Some(ref query_filter) = req.query {
                variables["filter"] = serde_json::json!({
                    "title": { "containsIgnoreCase": query_filter }
                });
            }

            let query = r#"
                query ListTeamIssues($teamId: String!, $first: Int!, $after: String, $filter: IssueFilter) {
                    team(id: $teamId) {
                        issues(first: $first, after: $after, filter: $filter, orderBy: updatedAt) {
                            nodes {
                                id
                                identifier
                                number
                                title
                                description
                                url
                                priority
                                priorityLabel
                                state {
                                    id
                                    name
                                    type
                                }
                                assignee {
                                    id
                                    name
                                    email
                                    avatarUrl
                                }
                                creator {
                                    id
                                    name
                                    email
                                    avatarUrl
                                }
                                team {
                                    id
                                    name
                                    key
                                }
                                labels {
                                    nodes {
                                        id
                                        name
                                        color
                                    }
                                }
                                createdAt
                                updatedAt
                                completedAt
                                cancelledAt
                            }
                            pageInfo {
                                hasNextPage
                                endCursor
                            }
                        }
                    }
                }
            "#;

            let data: TeamIssuesData = self.query(query, variables).await?;
            Ok(data.team.issues)
        } else {
            let mut variables = serde_json::json!({ "first": first });
            if let Some(ref after) = req.after {
                variables["after"] = serde_json::json!(after);
            }
            if let Some(ref query_filter) = req.query {
                variables["filter"] = serde_json::json!({
                    "title": { "containsIgnoreCase": query_filter }
                });
            }

            let query = r#"
                query ListIssues($first: Int!, $after: String, $filter: IssueFilter) {
                    issues(first: $first, after: $after, filter: $filter, orderBy: updatedAt) {
                        nodes {
                            id
                            identifier
                            number
                            title
                            description
                            url
                            priority
                            priorityLabel
                            state {
                                id
                                name
                                type
                            }
                            assignee {
                                id
                                name
                                email
                                avatarUrl
                            }
                            creator {
                                id
                                name
                                email
                                avatarUrl
                            }
                            team {
                                id
                                name
                                key
                            }
                            labels {
                                nodes {
                                    id
                                    name
                                    color
                                }
                            }
                            createdAt
                            updatedAt
                            completedAt
                            cancelledAt
                        }
                        pageInfo {
                            hasNextPage
                            endCursor
                        }
                    }
                }
            "#;

            let data: IssuesData = self.query(query, variables).await?;
            Ok(data.issues)
        }
    }
}
