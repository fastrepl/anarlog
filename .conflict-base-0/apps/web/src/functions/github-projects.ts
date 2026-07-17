import { env } from "../env";

const MARKETING_REPO_OWNER = "fastrepl";
const MARKETING_REPO_NAME = "marketing";

function getToken(): string | undefined {
  return env.GITHUB_TOKEN;
}

async function graphql(
  query: string,
  variables: Record<string, unknown>,
  token: string,
): Promise<unknown> {
  const response = await fetch("https://api.github.com/graphql", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ query, variables }),
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(`GitHub GraphQL error: ${response.status} - ${text}`);
  }

  const json = (await response.json()) as {
    data?: unknown;
    errors?: Array<{ message: string }>;
  };

  if (json.errors && json.errors.length > 0) {
    throw new Error(
      `GitHub GraphQL error: ${json.errors.map((e) => e.message).join(", ")}`,
    );
  }

  return json.data;
}

export interface ProjectV2 {
  id: string;
  title: string;
  number: number;
  url: string;
}

export interface StatusOption {
  id: string;
  name: string;
}

export interface ProjectItem {
  id: string;
  issueId: string;
  issueNumber: number;
  title: string;
  body: string;
  status: string | null;
  url: string;
  labels: string[];
  assignees: string[];
  createdAt: string;
  updatedAt: string;
}

export async function listProjects(): Promise<{
  projects: ProjectV2[];
  error?: string;
}> {
  const token = getToken();
  if (!token) {
    return { projects: [], error: "GitHub token not configured" };
  }

  const query = `
    query($owner: String!, $name: String!) {
      repository(owner: $owner, name: $name) {
        projectsV2(first: 20) {
          nodes {
            id
            title
            number
            url
          }
        }
      }
    }
  `;

  const data = (await graphql(
    query,
    {
      owner: MARKETING_REPO_OWNER,
      name: MARKETING_REPO_NAME,
    },
    token,
  )) as {
    repository: {
      projectsV2: {
        nodes: Array<{
          id: string;
          title: string;
          number: number;
          url: string;
        }>;
      };
    };
  };

  return {
    projects: data.repository.projectsV2.nodes.map((p) => ({
      id: p.id,
      title: p.title,
      number: p.number,
      url: p.url,
    })),
  };
}

export async function getProjectStatusField(projectId: string): Promise<{
  fieldId: string;
  options: StatusOption[];
  error?: string;
}> {
  const token = getToken();
  if (!token) {
    return { fieldId: "", options: [], error: "GitHub token not configured" };
  }

  const query = `
    query($projectId: ID!) {
      node(id: $projectId) {
        ... on ProjectV2 {
          fields(first: 30) {
            nodes {
              ... on ProjectV2SingleSelectField {
                id
                name
                options {
                  id
                  name
                }
              }
            }
          }
        }
      }
    }
  `;

  const data = (await graphql(query, { projectId }, token)) as {
    node: {
      fields: {
        nodes: Array<{
          id?: string;
          name?: string;
          options?: Array<{ id: string; name: string }>;
        }>;
      };
    };
  };

  const statusField = data.node.fields.nodes.find(
    (f) => f.name === "Status" && f.options,
  );

  if (!statusField || !statusField.id || !statusField.options) {
    return { fieldId: "", options: [], error: "Status field not found" };
  }

  return {
    fieldId: statusField.id,
    options: statusField.options.map((o) => ({ id: o.id, name: o.name })),
  };
}

export async function getProjectItems(
  projectNumber: number,
): Promise<{ items: ProjectItem[]; error?: string }> {
  const token = getToken();
  if (!token) {
    return { items: [], error: "GitHub token not configured" };
  }

  const query = `
    query($owner: String!, $name: String!, $projectNumber: Int!) {
      repository(owner: $owner, name: $name) {
        projectV2(number: $projectNumber) {
          items(first: 100) {
            nodes {
              id
              fieldValueByName(name: "Status") {
                ... on ProjectV2ItemFieldSingleSelectValue {
                  name
                }
              }
              content {
                ... on Issue {
                  id
                  number
                  title
                  body
                  url
                  createdAt
                  updatedAt
                  labels(first: 10) {
                    nodes {
                      name
                    }
                  }
                  assignees(first: 5) {
                    nodes {
                      login
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  `;

  const data = (await graphql(
    query,
    {
      owner: MARKETING_REPO_OWNER,
      name: MARKETING_REPO_NAME,
      projectNumber,
    },
    token,
  )) as {
    repository: {
      projectV2: {
        items: {
          nodes: Array<{
            id: string;
            fieldValueByName: { name?: string } | null;
            content: {
              id: string;
              number: number;
              title: string;
              body: string;
              url: string;
              createdAt: string;
              updatedAt: string;
              labels: { nodes: Array<{ name: string }> };
              assignees: { nodes: Array<{ login: string }> };
            } | null;
          }>;
        };
      };
    };
  };

  const items = data.repository.projectV2.items.nodes
    .filter((item) => item.content !== null)
    .map((item) => ({
      id: item.id,
      issueId: item.content!.id,
      issueNumber: item.content!.number,
      title: item.content!.title,
      body: item.content!.body,
      status: item.fieldValueByName?.name ?? null,
      url: item.content!.url,
      labels: item.content!.labels.nodes.map((l) => l.name),
      assignees: item.content!.assignees.nodes.map((a) => a.login),
      createdAt: item.content!.createdAt,
      updatedAt: item.content!.updatedAt,
    }));

  return { items };
}

export async function createIssue(
  title: string,
  body: string,
  labels?: string[],
): Promise<{
  issue?: { id: string; number: number; url: string };
  error?: string;
}> {
  const token = getToken();
  if (!token) {
    return { error: "GitHub token not configured" };
  }

  const repoQuery = `
    query($owner: String!, $name: String!) {
      repository(owner: $owner, name: $name) {
        id
        labels(first: 50) {
          nodes {
            id
            name
          }
        }
      }
    }
  `;

  const repoData = (await graphql(
    repoQuery,
    { owner: MARKETING_REPO_OWNER, name: MARKETING_REPO_NAME },
    token,
  )) as {
    repository: {
      id: string;
      labels: { nodes: Array<{ id: string; name: string }> };
    };
  };

  const labelIds = labels
    ? repoData.repository.labels.nodes
        .filter((l) => labels.includes(l.name))
        .map((l) => l.id)
    : [];

  const mutation = `
    mutation($repositoryId: ID!, $title: String!, $body: String!, $labelIds: [ID!]) {
      createIssue(input: {
        repositoryId: $repositoryId,
        title: $title,
        body: $body,
        labelIds: $labelIds
      }) {
        issue {
          id
          number
          url
        }
      }
    }
  `;

  const data = (await graphql(
    mutation,
    {
      repositoryId: repoData.repository.id,
      title,
      body,
      labelIds: labelIds.length > 0 ? labelIds : null,
    },
    token,
  )) as {
    createIssue: {
      issue: { id: string; number: number; url: string };
    };
  };

  return { issue: data.createIssue.issue };
}

export async function addIssueToProject(
  projectId: string,
  issueId: string,
): Promise<{ itemId?: string; error?: string }> {
  const token = getToken();
  if (!token) {
    return { error: "GitHub token not configured" };
  }

  const mutation = `
    mutation($projectId: ID!, $contentId: ID!) {
      addProjectV2ItemById(input: {
        projectId: $projectId,
        contentId: $contentId
      }) {
        item {
          id
        }
      }
    }
  `;

  const data = (await graphql(
    mutation,
    { projectId, contentId: issueId },
    token,
  )) as {
    addProjectV2ItemById: { item: { id: string } };
  };

  return { itemId: data.addProjectV2ItemById.item.id };
}

export async function updateItemStatus(
  projectId: string,
  itemId: string,
  fieldId: string,
  optionId: string,
): Promise<{ error?: string }> {
  const token = getToken();
  if (!token) {
    return { error: "GitHub token not configured" };
  }

  const mutation = `
    mutation($projectId: ID!, $itemId: ID!, $fieldId: ID!, $optionId: String!) {
      updateProjectV2ItemFieldValue(input: {
        projectId: $projectId,
        itemId: $itemId,
        fieldId: $fieldId,
        value: { singleSelectOptionId: $optionId }
      }) {
        projectV2Item {
          id
        }
      }
    }
  `;

  await graphql(mutation, { projectId, itemId, fieldId, optionId }, token);

  return {};
}

export async function updateIssue(
  issueId: string,
  title?: string,
  body?: string,
): Promise<{ error?: string }> {
  const token = getToken();
  if (!token) {
    return { error: "GitHub token not configured" };
  }

  const mutation = `
    mutation($issueId: ID!, $title: String, $body: String) {
      updateIssue(input: {
        id: $issueId,
        title: $title,
        body: $body
      }) {
        issue {
          id
        }
      }
    }
  `;

  await graphql(mutation, { issueId, title, body }, token);
  return {};
}

export async function closeIssue(issueId: string): Promise<{ error?: string }> {
  const token = getToken();
  if (!token) {
    return { error: "GitHub token not configured" };
  }

  const mutation = `
    mutation($issueId: ID!) {
      closeIssue(input: { issueId: $issueId }) {
        issue {
          id
        }
      }
    }
  `;

  await graphql(mutation, { issueId }, token);
  return {};
}

export async function deleteProjectItem(
  projectId: string,
  itemId: string,
): Promise<{ error?: string }> {
  const token = getToken();
  if (!token) {
    return { error: "GitHub token not configured" };
  }

  const mutation = `
    mutation($projectId: ID!, $itemId: ID!) {
      deleteProjectV2Item(input: {
        projectId: $projectId,
        itemId: $itemId
      }) {
        deletedItemId
      }
    }
  `;

  await graphql(mutation, { projectId, itemId }, token);
  return {};
}
