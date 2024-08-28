-- Add migration script here
CREATE TABLE newsletter_issues (
    newsletter_issue_id TEXT NOT NULL,
    title TEXT NOT NULL,
    text_content TEXT NOT NULL,
    html_content TEXT NOT NULL,
    published_at TEXT NOT NULL,
    PRIMARY KEY(newsletter_issue_id)
);