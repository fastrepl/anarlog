use anlg_apple_todo::types::{Reminder, ReminderList};
use anlg_ticket_interface::{CollectionPage, TicketPage};

use crate::error::Error;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum ReadPathResult {
    ReminderLists(Vec<ReminderList>),
    Reminders(Vec<Reminder>),
    Collections(CollectionPage),
    Tickets(TicketPage),
}

pub enum ReadPath<'a> {
    Apple(&'a str),
    LinearTeams {
        connection_id: &'a str,
    },
    LinearTickets {
        connection_id: &'a str,
        team_id: &'a str,
    },
    GithubRepos {
        connection_id: &'a str,
    },
    GithubTickets {
        connection_id: &'a str,
        owner: &'a str,
        repo: &'a str,
    },
}

impl<'a> ReadPath<'a> {
    pub fn parse(path: &'a str) -> Result<Self, Error> {
        let trimmed = path.trim_matches('/');
        let segments = trimmed
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();

        match segments.as_slice() {
            ["apple"] => Ok(Self::Apple("")),
            ["apple", ..] => Ok(Self::Apple(&trimmed["apple/".len()..])),
            ["linear", connection_id]
            | ["linear", connection_id, "collections"]
            | ["linear", connection_id, "teams"] => Ok(Self::LinearTeams { connection_id }),
            ["linear", connection_id, "teams", team_id]
            | ["linear", connection_id, "teams", team_id, "tickets"] => Ok(Self::LinearTickets {
                connection_id,
                team_id,
            }),
            ["github", connection_id]
            | ["github", connection_id, "collections"]
            | ["github", connection_id, "repos"] => Ok(Self::GithubRepos { connection_id }),
            ["github", connection_id, "repos", owner, repo]
            | ["github", connection_id, "repos", owner, repo, "tickets"] => {
                Ok(Self::GithubTickets {
                    connection_id,
                    owner,
                    repo,
                })
            }
            _ => Err(Error::InvalidReadPath(path.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ReadPath;

    #[test]
    fn parses_supported_paths() {
        for path in [
            "apple",
            "linear/conn-1/teams/team-1",
            "/linear/conn-1/teams/team-1/tickets/",
            "github/conn-1/repos/openai/char",
            "github/conn-1/repos/openai/char/tickets",
            "linear/conn-1/collections",
            "github/conn-1/collections",
        ] {
            let parsed = ReadPath::parse(path).unwrap();
            assert!(
                match path {
                    "apple" => matches!(parsed, ReadPath::Apple("")),
                    "linear/conn-1/teams/team-1" | "/linear/conn-1/teams/team-1/tickets/" => {
                        matches!(
                            parsed,
                            ReadPath::LinearTickets {
                                connection_id: "conn-1",
                                team_id: "team-1"
                            }
                        )
                    }
                    "github/conn-1/repos/openai/char"
                    | "github/conn-1/repos/openai/char/tickets" => matches!(
                        parsed,
                        ReadPath::GithubTickets {
                            connection_id: "conn-1",
                            owner: "openai",
                            repo: "char"
                        }
                    ),
                    "linear/conn-1/collections" => matches!(
                        parsed,
                        ReadPath::LinearTeams {
                            connection_id: "conn-1"
                        }
                    ),
                    "github/conn-1/collections" => matches!(
                        parsed,
                        ReadPath::GithubRepos {
                            connection_id: "conn-1"
                        }
                    ),
                    _ => false,
                },
                "unexpected path variant for {path}"
            );
        }
    }

    #[test]
    fn rejects_unknown_paths() {
        assert!(ReadPath::parse("linear").is_err());
        assert!(ReadPath::parse("github/conn-1/repos/openai/char/pulls").is_err());
    }
}
