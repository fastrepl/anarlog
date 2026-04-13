import type { TaskType } from "~/store/tinybase/store/prompts";

const ENHANCE_STARTER_TEMPLATE = `
# Context

{% if session.title %}
Title: {{ session.title }}
{% endif %}
{% if session.startedAt %}
Date: {{ session.startedAt }}
{% endif %}

{% if participants %}
Participants:
{% for participant in participants %}
- {{ participant.name }}{% if participant.jobTitle %} - {{ participant.jobTitle }}{% endif %}
{% endfor %}
{% endif %}

{% if pre_meeting_memo %}
# Pre-Meeting Notes

{{ pre_meeting_memo }}
{% endif %}

{% if post_meeting_memo %}
# Meeting Notes

{{ post_meeting_memo }}
{% endif %}

# Transcript

{{ content | transcript }}

{% if template and template.sections %}
# Output Template

{% for section in template.sections %}
{{ loop.index }}. {{ section.title }}{% if section.description %} - {{ section.description }}{% endif %}
{% endfor %}
{% endif %}
`.trim();

const TITLE_STARTER_TEMPLATE = `
<note>
{{ enhanced_note }}
</note>

Give me a super concise meeting title. Focus only on the topic and return title text only.
`.trim();

const DEFAULT_PROMPT_TEMPLATES = {
  enhance: ENHANCE_STARTER_TEMPLATE,
  title: TITLE_STARTER_TEMPLATE,
} satisfies Record<TaskType, string>;

export function getDefaultPromptTemplate(taskType: TaskType): string {
  return DEFAULT_PROMPT_TEMPLATES[taskType];
}
