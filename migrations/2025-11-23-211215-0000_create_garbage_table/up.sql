CREATE TABLE garbage (
    id SERIAL PRIMARY KEY,
    garbage_name VARCHAR NOT NULL
);

INSERT INTO garbage (garbage_name) VALUES
    ('banana peel'),
    ('coffee grounds');
