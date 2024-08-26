-- Add migration script here
INSERT INTO users (user_id, username, password_hash)
VALUES (
    '0H2CE35DCZ9JG',
    'admin',
    '$argon2id$v=19$m=19456,t=2,p=1$plKP8RnQ/nXTuki8Lhxpqw$ZJKuAuqoiI9GiT45KSNl6mKJMRMF4VGzp7SBd7AWatc'
);