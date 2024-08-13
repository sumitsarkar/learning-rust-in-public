-- Add migration script here
CREATE TABLE subscription_tokens(
    subscription_token TEXT NOT NULL,
    subscriber_id TEXT NOT NULL,
    PRIMARY KEY (subscription_token),
    FOREIGN KEY (subscriber_id) REFERENCES subscriptions(id)
);
