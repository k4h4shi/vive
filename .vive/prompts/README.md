# Prompt Templates

This directory contains customizable prompt templates for vive. You can modify existing templates or create new ones to suit your project's needs.

## Available Templates

- `issue.txt` - Default template for issue fixing (Japanese)
- `default.txt` - English template for issue fixing

## Creating Custom Templates

1. Create a new `.txt` file in this directory
2. Update the `template` field in `..config.json` to use your new template
3. Use the available template variables listed below

## Template Variables

The following variables are available for substitution in your templates:

- `{{ISSUE_NUMBER}}` - The GitHub issue number
- `{{ISSUE_TITLE}}` - The issue title
- `{{ISSUE_BODY}}` - The complete issue body
- `{{ISSUE_BODY_TRUNCATED}}` - Issue body truncated to 1000 characters
- `{{WORKTREE_DIR}}` - The path to the working directory
- `{{CONTEXT_NOTE}}` - Additional context (e.g., continuation job information)

## Configuration

Edit `.vive/config.json` to change the template:

```json
{
  "prompts": {
    "template": "your-template-name",
    "customFields": {}
  }
}
```

## Example Custom Template

```
Task: {{ISSUE_TITLE}}

Description:
{{ISSUE_BODY_TRUNCATED}}

Working in: {{WORKTREE_DIR}}

Please implement the requested changes following our coding standards.
```