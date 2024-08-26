-- Add migration script here
INSERT INTO users (user_id, username, password_hash)
VALUES (
    '0H2CE35DCZ9JG',
    'admin',
    '$argon2id$v=19$m=19456,t=2,p=1$rMGTDhHiGlQ2+VCPhSOmXw$MkjJwmHgBn3WYeWejWAVmBtfRMcQggeJ2SrQAFfpUUI'
);